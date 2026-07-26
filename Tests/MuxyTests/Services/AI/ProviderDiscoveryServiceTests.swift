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
    func processRunnerTerminatesTimeout() async {
        let clock = ContinuousClock()
        let startedAt = clock.now

        await #expect(throws: ProviderDiscoveryError.self) {
            try await ProviderDiscoveryService.runProcess(
                executablePath: "/bin/sh",
                arguments: ["-c", "exec sleep 5"],
                workingDirectory: "/tmp",
                timeout: 0.05
            )
        }

        #expect(startedAt.duration(to: clock.now) < .seconds(4))
    }
}

private func providerDiscoveryProcessResult(
    status: Int32 = 0,
    stdout: String = "",
    stderr: String = ""
) -> GitProcessResult {
    GitProcessResult(
        status: status,
        stdout: stdout,
        stdoutData: Data(stdout.utf8),
        stderr: stderr,
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

    init(executablePath: String?, details: ProviderDiscoveryDetails) {
        self.executablePath = executablePath
        self.details = details
    }

    func isToolInstalled() -> Bool { executablePath != nil }
    func agentCLIExecutablePath() -> String? { executablePath }
    func install(hookScriptPath _: String) throws {}
    func uninstall() throws {}
    func discoveryDetails(from _: String) -> ProviderDiscoveryDetails { details }

    func resetSettings() {
        UserDefaults.standard.removeObject(forKey: settingsKey)
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
