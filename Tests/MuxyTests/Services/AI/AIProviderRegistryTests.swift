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

    @Test("notificationSource resolves every provider socket type to its id")
    func notificationSourceResolvesEveryProvider() {
        let expected: [String: String] = [
            "claude_hook": "claude",
            "cursor_hook": "cursor",
            "codex_hook": "codex",
            "droid_hook": "droid",
            "opencode": "opencode",
            "pi": "pi",
            "grok_hook": "grok",
        ]
        for (socketType, providerID) in expected {
            #expect(AIProviderRegistry.shared.notificationSource(for: socketType) == .aiProvider(providerID))
        }
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

    @Test("installAll refreshes only providers whose hook is already installed in dev")
    func installAllRefreshesInstalledHooksInDev() async {
        let installed = RefreshRecordingProvider()
        let notInstalled = RefreshRecordingProvider()
        defer {
            installed.resetSettings()
            notInstalled.resetSettings()
        }
        installed.isEnabled = true
        installed.hookInstalled = true
        notInstalled.isEnabled = true
        notInstalled.hookInstalled = false

        let registry = AIProviderRegistry(
            providers: [installed, notInstalled],
            hydrateLoginShellPath: {},
            shouldInstallHooksInDebug: { false }
        )

        await registry.installAll()

        #expect(installed.hookInstalledCheckCount >= 1)
        #expect(!notInstalled.installAttempted)
    }
}

private final class RefreshRecordingProvider: AIProviderIntegration {
    let id = "refresh-recording-provider-\(UUID().uuidString)"
    let displayName = "Refresh Recording Provider"
    let socketTypeKey = "refresh_recording"
    let iconName = "sparkles"
    let executableNames = ["refresh-recording"]
    var hookInstalled = false
    var hookInstalledCheckCount = 0
    var installAttempted = false

    func isToolInstalled() -> Bool { false }

    func isHookInstalled() -> Bool {
        hookInstalledCheckCount += 1
        return hookInstalled
    }

    func install(hookScriptPath _: String) throws {
        installAttempted = true
    }

    func uninstall() throws {}

    func resetSettings() {
        UserDefaults.standard.removeObject(forKey: settingsKey)
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
