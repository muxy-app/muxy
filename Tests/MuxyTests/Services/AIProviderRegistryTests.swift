import Foundation
import Testing

@testable import Muxy

@Suite("AIProviderRegistry")
@MainActor
struct AIProviderRegistryTests {
    @Test("notificationSource resolves built-in socket type keys")
    func notificationSourceResolvesBuiltIn() {
        let source = AIProviderRegistry.shared.notificationSource(for: "claude_hook")
        #expect(source == .aiProvider("claude"))
    }

    @Test("notificationSource falls back to socket for unknown types")
    func notificationSourceFallsBackToSocket() {
        let source = AIProviderRegistry.shared.notificationSource(for: "not-a-known-type")
        #expect(source == .socket)
    }

    @Test("iconName resolves a built-in provider icon")
    func iconNameResolvesBuiltIn() {
        #expect(AIProviderRegistry.shared.iconName(for: .aiProvider("claude")) == "claude")
    }

    @Test("iconName falls back to sparkles for an extension source")
    func iconNameFallsBackForExtension() {
        #expect(AIProviderRegistry.shared.iconName(for: .aiProvider("some-extension")) == "sparkles")
    }

    @Test("iconName resolves osc and socket sources")
    func iconNameResolvesStaticSources() {
        #expect(AIProviderRegistry.shared.iconName(for: .osc) == "terminal")
        #expect(AIProviderRegistry.shared.iconName(for: .socket) == "network")
    }

    @Test("installAll waits for login shell PATH hydration before checking providers")
    func installAllWaitsForLoginShellPathHydrationBeforeCheckingProviders() async {
        let provider = RecordingProvider()
        defer { provider.resetSettings() }
        provider.isEnabled = true
        let gate = HydrationGate()
        let registry = AIProviderRegistry(
            providers: [provider],
            hydrateLoginShellPath: { await gate.wait() },
            shouldInstallHooksInDebug: { true }
        )

        let installTask = Task {
            await registry.installAll()
        }
        while !gate.started {
            await Task.yield()
        }

        #expect(provider.toolCheckCount == 0)
        gate.finish()
        await installTask.value
        #expect(provider.toolCheckCount == 1)
    }

    @Test("installAll uninstalls disabled providers without login shell PATH hydration")
    func installAllUninstallsDisabledProvidersWithoutLoginShellPathHydration() async {
        let provider = RecordingProvider()
        defer { provider.resetSettings() }
        provider.isEnabled = false
        let gate = HydrationGate()
        let registry = AIProviderRegistry(
            providers: [provider],
            hydrateLoginShellPath: { await gate.wait() },
            shouldInstallHooksInDebug: { true }
        )

        await registry.installAll()

        #expect(provider.uninstallCount == 1)
        #expect(!gate.started)
    }
}

private final class RecordingProvider: AIProviderIntegration {
    let id: String
    let displayName = "Registry Test Provider"
    let socketTypeKey = "registry_test"
    let iconName = "sparkles"
    let executableNames = ["registry-test"]
    var toolCheckCount = 0
    var uninstallCount = 0

    init(id: String = "registry-test-provider-\(UUID().uuidString)") {
        self.id = id
    }

    func isToolInstalled() -> Bool {
        toolCheckCount += 1
        return false
    }

    func install(hookScriptPath _: String) throws {}

    func uninstall() throws {
        uninstallCount += 1
    }

    func resetSettings() {
        UserDefaults.standard.removeObject(forKey: settingsKey)
    }
}

private final class HydrationGate: @unchecked Sendable {
    private let lock = NSLock()
    private var continuation: CheckedContinuation<Void, Never>?
    private var didStart = false

    var started: Bool {
        lock.withLock { didStart }
    }

    func wait() async {
        await withCheckedContinuation { continuation in
            lock.withLock {
                didStart = true
                self.continuation = continuation
            }
        }
    }

    func finish() {
        let pending = lock.withLock {
            let pending = continuation
            continuation = nil
            return pending
        }
        pending?.resume()
    }
}
