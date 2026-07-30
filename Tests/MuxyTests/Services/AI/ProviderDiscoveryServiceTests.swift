import Foundation
import Testing

@testable import Muxy

@Suite("ProviderDiscoveryService")
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

        await #expect(throws: SubprocessRunnerError.self) {
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
        await #expect(throws: Error.self) {
            try await SubprocessRunner.run(SubprocessRequest(executablePath: "/missing/muxy-probe"))
        }
    }

    @Test("process runner cancels the process group")
    func processRunnerCancelsProcessGroup() async throws {
        let task = Task {
            try await SubprocessRunner.run(SubprocessRequest(
                executablePath: "/bin/sh",
                arguments: ["-c", "(/bin/sleep 10) & wait"]
            ))
        }
        try await Task.sleep(for: .milliseconds(50))
        task.cancel()

        await #expect(throws: Error.self) {
            try await task.value
        }
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
