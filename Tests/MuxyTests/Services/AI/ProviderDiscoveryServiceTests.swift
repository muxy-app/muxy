import Foundation
import Testing

@testable import Muxy

@Suite("ProviderDiscoveryService", .serialized)
@MainActor
struct ProviderDiscoveryServiceTests {
    @Test("discovery records executable version and ready state")
    func recordsReadyDiscovery() async {
        let provider = DiscoveryProvider(
            executablePath: "/tmp/opencode",
            details: ProviderDiscoveryDetails(version: "1.18.5", state: .ready)
        )
        defer { provider.resetSettings() }
        let health = HookHealthStore()
        let recorder = InvocationRecorder()
        let service = ProviderDiscoveryService(health: health) { executable, arguments, directory, timeout in
            recorder.record(executable, arguments, directory, timeout)
            return providerDiscoveryProcessResult(stdout: "diagnostic output")
        }

        await service.discover(provider)

        #expect(recorder.executable == "/tmp/opencode")
        #expect(recorder.arguments == ["debug", "info"])
        #expect(recorder.workingDirectory == "/tmp")
        #expect(recorder.timeout == ProviderDiscoveryService.defaultTimeout)
        #expect(health.health(for: provider.id).discovery == ProviderDiscoverySnapshot(
            executablePath: "/tmp/opencode",
            version: "1.18.5",
            state: .ready
        ))
        #expect(health.health(for: provider.id).lastDiscoveredAt != nil)
    }

    @Test("discovery records command failures without changing hook state")
    func recordsCommandFailure() async {
        let provider = DiscoveryProvider(
            executablePath: "/tmp/opencode",
            details: ProviderDiscoveryDetails(version: nil, state: .ready)
        )
        defer { provider.resetSettings() }
        let health = HookHealthStore()
        health.noteVerified(providerID: provider.id, state: .installed)
        let service = ProviderDiscoveryService(health: health) { _, _, _, _ in
            providerDiscoveryProcessResult(status: 2, stderr: "invalid config")
        }

        await service.discover(provider)

        #expect(health.health(for: provider.id).installState == .installed)
        #expect(health.health(for: provider.id).discovery == ProviderDiscoverySnapshot(
            executablePath: "/tmp/opencode",
            version: nil,
            state: .failed("invalid config")
        ))
    }

    @Test("discovery records a missing executable")
    func recordsMissingExecutable() async {
        let provider = DiscoveryProvider(
            executablePath: nil,
            details: ProviderDiscoveryDetails(version: nil, state: .ready)
        )
        defer { provider.resetSettings() }
        let health = HookHealthStore()
        let service = ProviderDiscoveryService(health: health) { _, _, _, _ in
            Issue.record("Runner should not execute without a CLI")
            return providerDiscoveryProcessResult()
        }

        await service.discover(provider)

        #expect(health.health(for: provider.id).discovery == ProviderDiscoverySnapshot(
            executablePath: nil,
            version: nil,
            state: .failed("CLI executable not found")
        ))
    }

    @Test("cancelled discovery preserves the previous result")
    func cancelledDiscoveryPreservesPreviousResult() async {
        let provider = DiscoveryProvider(
            executablePath: "/tmp/opencode",
            details: ProviderDiscoveryDetails(version: nil, state: .ready)
        )
        defer { provider.resetSettings() }
        let health = HookHealthStore()
        let previous = ProviderDiscoverySnapshot(
            executablePath: "/tmp/opencode",
            version: "1.18.5",
            state: .ready
        )
        health.noteDiscovery(providerID: provider.id, snapshot: previous)
        let service = ProviderDiscoveryService(health: health) { _, _, _, _ in
            throw SubprocessRunnerError.cancelled
        }

        await service.discover(provider)

        #expect(health.health(for: provider.id).discovery == previous)
    }

    @Test("process runner captures bounded standard output and error")
    func processRunnerCapturesOutput() async throws {
        let result = try await ProviderDiscoveryService.runProcess(
            executablePath: "/bin/sh",
            arguments: ["-c", "printf output; printf error >&2"],
            workingDirectory: "/tmp",
            timeout: 1
        )

        #expect(result.status == 0)
        #expect(result.stdout == "output")
        #expect(result.stderr == "error")
        #expect(!result.truncated)
    }

    @Test("process runner terminates timed out probes")
    func processRunnerTerminatesTimeout() async throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("ProviderDiscoveryTimeoutTests-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let completionMarker = directory.appendingPathComponent("completed").path

        await #expect(throws: SubprocessRunnerError.timedOut(0.05)) {
            try await ProviderDiscoveryService.runProcess(
                executablePath: "/bin/sh",
                arguments: ["-c", "/bin/sleep 1; /usr/bin/touch '\(completionMarker)'"],
                workingDirectory: directory.path,
                timeout: 0.05
            )
        }

        #expect(!FileManager.default.fileExists(atPath: completionMarker))
    }

    @Test("process runner drains and truncates oversized output")
    func processRunnerTruncatesOutput() async throws {
        let result = try await ProviderDiscoveryService.runProcess(
            executablePath: "/bin/sh",
            arguments: ["-c", "/usr/bin/yes x | /usr/bin/head -c 200000"],
            workingDirectory: "/tmp",
            timeout: 2
        )

        #expect(result.status == 0)
        #expect(result.truncated)
        #expect(result.stdoutData.count == 64 * 1024)
    }

    @Test("process runner executes independent probes concurrently")
    func processRunnerExecutesConcurrently() async throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("ProviderDiscoveryConcurrencyTests-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let firstMarker = directory.appendingPathComponent("first").path
        let secondMarker = directory.appendingPathComponent("second").path

        async let first = ProviderDiscoveryService.runProcess(
            executablePath: "/bin/sh",
            arguments: [
                "-c",
                "/usr/bin/touch '\(firstMarker)'; while [ ! -e '\(secondMarker)' ]; do /bin/sleep 0.01; done",
            ],
            workingDirectory: directory.path,
            timeout: 1
        )
        async let second = ProviderDiscoveryService.runProcess(
            executablePath: "/bin/sh",
            arguments: [
                "-c",
                "/usr/bin/touch '\(secondMarker)'; while [ ! -e '\(firstMarker)' ]; do /bin/sleep 0.01; done",
            ],
            workingDirectory: directory.path,
            timeout: 1
        )

        let results = try await (first, second)
        #expect(results.0.status == 0)
        #expect(results.1.status == 0)
    }

    @Test("process runner reports launch failures")
    func processRunnerReportsLaunchFailure() async {
        do {
            _ = try await SubprocessRunner.run(SubprocessRequest(executablePath: "/missing/muxy-probe"))
            Issue.record("Expected the process launch to fail")
        } catch let error as SubprocessRunnerError {
            guard case .launchFailed = error else {
                Issue.record("Expected a launch failure, received \(error)")
                return
            }
        } catch {
            Issue.record("Expected SubprocessRunnerError, received \(error)")
        }
    }

    @Test("process runner finishes when cancellation precedes execution")
    func processRunnerHandlesPreCancelledTask() async {
        let gate = ProbeGate()
        let task = Task {
            await gate.wait()
            return try await SubprocessRunner.run(SubprocessRequest(executablePath: "/usr/bin/true"))
        }
        await gate.waitUntilReady()
        task.cancel()
        await gate.open()

        await #expect(throws: SubprocessRunnerError.cancelled) {
            try await task.value
        }
    }

    @Test("process runner cancels the process group")
    func processRunnerCancelsProcessGroup() async throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("SubprocessCancellationTests-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let pidFile = directory.appendingPathComponent("child-pid")
        let task = Task {
            try await SubprocessRunner.run(SubprocessRequest(
                executablePath: "/bin/sh",
                arguments: ["-c", "/bin/sleep 10 & child=$!; echo $child > '\(pidFile.path)'; wait"],
                standardInput: Data(repeating: 0, count: 1024 * 1024)
            ))
        }
        try await waitForFile(at: pidFile)
        let pidContents = try String(contentsOf: pidFile, encoding: .utf8)
        let childPID = try #require(pid_t(pidContents.trimmingCharacters(in: .whitespacesAndNewlines)))
        task.cancel()

        await #expect(throws: SubprocessRunnerError.cancelled) {
            try await task.value
        }
        #expect(await waitForProcessExit(childPID))
    }

    @Test("newer discovery results replace older probes")
    func newerDiscoveryResultWins() async {
        let provider = DiscoveryProvider(
            executablePath: "/tmp/opencode",
            details: ProviderDiscoveryDetails(version: nil, state: .ready),
            usesOutputAsVersion: true
        )
        defer { provider.resetSettings() }
        let health = HookHealthStore()
        let sequence = ProbeSequence()
        let service = ProviderDiscoveryService(health: health) { _, _, _, _ in
            let invocation = await sequence.next()
            if invocation == 1 {
                try await Task.sleep(for: .milliseconds(100))
                return providerDiscoveryProcessResult(stdout: "old")
            }
            return providerDiscoveryProcessResult(stdout: "new")
        }

        let first = Task { @MainActor in
            await service.discover(provider)
        }
        try? await Task.sleep(for: .milliseconds(10))
        let second = Task { @MainActor in
            await service.discover(provider)
        }
        await first.value
        await second.value

        #expect(health.health(for: provider.id).discovery?.version == "new")
    }
}

private func providerDiscoveryProcessResult(
    status: Int32 = 0,
    stdout: String = "",
    stderr: String = ""
) -> SubprocessResult {
    SubprocessResult(
        status: status,
        stdoutData: Data(stdout.utf8),
        stderrData: Data(stderr.utf8),
        truncated: false
    )
}

private final class DiscoveryProvider: AIProviderIntegration, AIAgentLaunchProvider, AIProviderDiscoveryIntegration {
    let id = "discovery-provider-\(UUID().uuidString)"
    let displayName = "Discovery Provider"
    let socketTypeKey = "discovery_provider"
    let iconName = "sparkles"
    let executableNames = ["discovery-provider"]
    let agentLaunchConfiguration = AIAgentLaunchConfiguration(
        executable: "discovery-provider",
        headlessArguments: []
    )
    let discoveryArguments = ["debug", "info"]
    let discoveryWorkingDirectory = "/tmp"
    private let executablePath: String?
    private let details: ProviderDiscoveryDetails
    private let usesOutputAsVersion: Bool

    init(
        executablePath: String?,
        details: ProviderDiscoveryDetails,
        usesOutputAsVersion: Bool = false
    ) {
        self.executablePath = executablePath
        self.details = details
        self.usesOutputAsVersion = usesOutputAsVersion
    }

    func isToolInstalled() -> Bool { executablePath != nil }
    func agentCLIExecutablePath() -> String? { executablePath }
    func install(hookScriptPath _: String) throws {}
    func uninstall() throws {}
    func discoveryDetails(from output: String) -> ProviderDiscoveryDetails {
        guard usesOutputAsVersion else { return details }
        return ProviderDiscoveryDetails(version: output, state: details.state)
    }

    func resetSettings() {
        UserDefaults.standard.removeObject(forKey: settingsKey)
    }
}

private actor ProbeSequence {
    private var value = 0

    func next() -> Int {
        value += 1
        return value
    }
}

private actor ProbeGate {
    private var continuation: CheckedContinuation<Void, Never>?
    private var ready = false

    func wait() async {
        ready = true
        await withCheckedContinuation { continuation = $0 }
    }

    func waitUntilReady() async {
        while !ready {
            await Task.yield()
        }
    }

    func open() {
        continuation?.resume()
        continuation = nil
    }
}

private struct ProbeMarkerTimeoutError: Error {}

private func waitForFile(at url: URL) async throws {
    for _ in 0 ..< 250 {
        if FileManager.default.fileExists(atPath: url.path) {
            return
        }
        try await Task.sleep(for: .milliseconds(20))
    }
    Issue.record("expected marker file at \(url.path)")
    throw ProbeMarkerTimeoutError()
}

private func waitForProcessExit(_ pid: pid_t) async -> Bool {
    for _ in 0 ..< 100 {
        if kill(pid, 0) == -1, errno == ESRCH {
            return true
        }
        try? await Task.sleep(for: .milliseconds(20))
    }
    return false
}

private final class InvocationRecorder: @unchecked Sendable {
    private let lock = NSLock()
    private var values: (String, [String], String, TimeInterval)?

    var executable: String? { lock.withLock { values?.0 } }
    var arguments: [String]? { lock.withLock { values?.1 } }
    var workingDirectory: String? { lock.withLock { values?.2 } }
    var timeout: TimeInterval? { lock.withLock { values?.3 } }

    func record(_ executable: String, _ arguments: [String], _ directory: String, _ timeout: TimeInterval) {
        lock.withLock { values = (executable, arguments, directory, timeout) }
    }
}
