import Foundation
import Testing

@testable import Muxy

@Suite("AIProviderRegistry")
@MainActor
struct AIProviderRegistryTests {
    private static func stubInstaller() -> HookInstaller {
        HookInstaller(
            hookScriptPath: { _, _ in "/tmp/muxy-test-hook" },
            stagedFileExists: { _ in true },
            stagedFileExecutable: { _ in true },
            health: HookHealthStore()
        )
    }

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

    @Test("metadata providers include every supported CLI")
    func agentLaunchProvidersIncludeEverySupportedCLI() {
        #expect(AIProviderRegistry.shared.agentLaunchProviders.map(\.id) == [
            "claude",
            "opencode",
            "codex",
            "cursor",
            "droid",
            "pi",
            "grok",
        ])
    }

    @Test("prepareForInstallation stages resources but skips PATH hydration without dev opt-in")
    func prepareForInstallationStagesWithoutDevOptIn() async {
        let staging = StagingRecorder()
        let registry = AIProviderRegistry(
            providers: [],
            hydrateLoginShellPath: {},
            shouldInstallHooksInDebug: { false },
            stageHookResources: {
                staging.record()
                return true
            }
        )

        registry.prepareForInstallation()
        await Task.yield()

        #expect(staging.count == 1)
    }

    @Test("installAll cleans disabled provider managed state without PATH hydration")
    func installAllCleansDisabledProviderManagedStateWithoutPathHydration() async {
        let provider = RecordingProvider()
        defer { provider.resetSettings() }
        provider.isEnabled = false
        provider.managedStateInstalled = true
        let registry = AIProviderRegistry(
            providers: [provider],
            hydrateLoginShellPath: {},
            shouldInstallHooksInDebug: { true },
            stageHookResources: { true }
        )

        await registry.installAll()

        #expect(provider.uninstallCount == 1)
        #expect(provider.toolCheckCount == 0)
    }

    @Test("installAll does not touch disabled providers without managed state")
    func installAllDoesNotTouchDisabledProvidersWithoutManagedState() async {
        let provider = RecordingProvider()
        defer { provider.resetSettings() }
        provider.isEnabled = false
        let registry = AIProviderRegistry(
            providers: [provider],
            hydrateLoginShellPath: {},
            shouldInstallHooksInDebug: { false }
        )

        await registry.installAll()

        #expect(provider.uninstallCount == 0)
        #expect(provider.toolCheckCount == 0)
    }

    @Test("installAll does not reconcile hooks in dev without explicit opt-in")
    func installAllDoesNotReconcileHooksInDevWithoutExplicitOptIn() async {
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
        notInstalled.toolInstalled = true

        let registry = AIProviderRegistry(
            providers: [installed, notInstalled],
            hydrateLoginShellPath: {},
            shouldInstallHooksInDebug: { false },
            hookScriptPath: { _, _ in "/tmp/muxy-test-hook" }
        )

        await registry.installAll()

        #expect(installed.hookInstalledCheckCount == 0)
        #expect(!installed.installAttempted)
        #expect(!notInstalled.installAttempted)
        #expect(notInstalled.toolCheckCount == 0)
    }

    @Test("prepare and install stage resources once before provider reconciliation")
    func stagingRunsOnceBeforeReconciliation() async {
        let provider = RefreshRecordingProvider()
        defer { provider.resetSettings() }
        provider.isEnabled = true
        provider.hookInstalled = false
        provider.toolInstalled = true
        let staging = StagingRecorder()
        let registry = AIProviderRegistry(
            providers: [provider],
            hydrateLoginShellPath: {},
            shouldInstallHooksInDebug: { true },
            hookScriptPath: { _, _ in "/tmp/muxy-test-hook" },
            stageHookResources: {
                staging.record()
                return true
            },
            installer: Self.stubInstaller()
        )

        registry.prepareForInstallation()
        await registry.installAll()

        #expect(staging.count == 1)
        #expect(provider.installAttempted)
    }

    @Test("force install restages hook resources before reinstalling")
    func forceInstallRestagesHookResources() async {
        let provider = RecordingProvider()
        defer { provider.resetSettings() }
        let staging = StagingRecorder()
        let registry = AIProviderRegistry(
            providers: [provider],
            hydrateLoginShellPath: {},
            shouldInstallHooksInDebug: { true },
            hookScriptPath: { _, _ in "/tmp/muxy-test-hook" },
            stageHookResources: {
                staging.record()
                return true
            },
            installer: Self.stubInstaller()
        )

        registry.prepareForInstallation()
        await registry.forceInstall(provider)

        #expect(staging.count == 2)
        #expect(provider.uninstallCount == 1)
        #expect(provider.installCount == 1)
    }

    @Test("installAll installs missing hooks in dev only with the explicit opt-in")
    func installAllInstallsMissingHooksInDevWithExplicitOptIn() async {
        let provider = RecordingProvider()
        defer { provider.resetSettings() }
        provider.isEnabled = true
        provider.toolInstalled = true
        let registry = AIProviderRegistry(
            providers: [provider],
            hydrateLoginShellPath: {},
            shouldInstallHooksInDebug: { true },
            hookScriptPath: { _, _ in "/tmp/muxy-test-hook" },
            stageHookResources: { true },
            installer: Self.stubInstaller()
        )

        await registry.installAll()

        #expect(provider.toolCheckCount == 1)
        #expect(provider.installCount == 1)
    }
}

private final class StagingRecorder: @unchecked Sendable {
    private let lock = NSLock()
    private var storage = 0

    var count: Int {
        lock.withLock { storage }
    }

    func record() {
        lock.withLock { storage += 1 }
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
    var toolInstalled = false
    var toolCheckCount = 0
    var installAttempted = false

    func isToolInstalled() -> Bool {
        toolCheckCount += 1
        return toolInstalled
    }

    func isHookInstalled() -> Bool {
        hookInstalledCheckCount += 1
        return hookInstalled
    }

    func install(hookScriptPath _: String) throws {
        installAttempted = true
        hookInstalled = true
    }

    func verify(hookScriptPath _: String) -> HookVerification {
        hookInstalled ? .satisfied : .needsRepair
    }

    func uninstall() throws {
        hookInstalled = false
    }

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
    var hookInstalled = false
    var managedStateInstalled = false
    var toolInstalled = false
    var toolCheckCount = 0
    var installCount = 0
    var uninstallCount = 0

    init(id: String = "registry-test-provider-\(UUID().uuidString)") {
        self.id = id
    }

    func isToolInstalled() -> Bool {
        toolCheckCount += 1
        return toolInstalled
    }

    func isHookInstalled() -> Bool {
        hookInstalled
    }

    func hasManagedState() -> Bool {
        managedStateInstalled || hookInstalled
    }

    func verify(hookScriptPath _: String) -> HookVerification {
        hookInstalled ? .satisfied : .needsRepair
    }

    func install(hookScriptPath _: String) throws {
        installCount += 1
        hookInstalled = true
    }

    func uninstall() throws {
        uninstallCount += 1
        hookInstalled = false
    }

    func resetSettings() {
        UserDefaults.standard.removeObject(forKey: settingsKey)
    }
}

