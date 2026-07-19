import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "AIProviderRegistry")

enum HookVerification: Equatable {
    case satisfied
    case needsRepair
    case conflict(String)
    case failed(String)
}

protocol AIProviderIntegration {
    var id: String { get }
    var displayName: String { get }
    var socketTypeKey: String { get }
    var iconName: String { get }
    var executableNames: [String] { get }
    var hookScriptName: String { get }
    var hookScriptExtension: String { get }
    var configPaths: [String] { get }

    func isToolInstalled() -> Bool
    func isHookInstalled() -> Bool
    func hasManagedState() -> Bool
    func install(hookScriptPath: String) throws
    func uninstall() throws
    func verify(hookScriptPath: String) -> HookVerification
}

extension AIProviderIntegration {
    func isHookInstalled() -> Bool {
        false
    }

    func hasManagedState() -> Bool {
        isHookInstalled()
    }

    var configPaths: [String] { [] }

    func verify(hookScriptPath _: String) -> HookVerification {
        isHookInstalled() ? .satisfied : .needsRepair
    }
}

extension AIProviderIntegration {
    var hookScriptName: String { "muxy-claude-hook" }
    var hookScriptExtension: String { "sh" }
}

extension AIProviderIntegration {
    var settingsKey: String { NotificationSettings.providerEnabledKey(for: id) }

    var isEnabled: Bool {
        get { NotificationSettings.providerEnabled(providerID: id) }
        nonmutating set { UserDefaults.standard.set(newValue, forKey: settingsKey) }
    }

    func isToolInstalled() -> Bool {
        let home = NSHomeDirectory()
        let searchPaths = executableNames.flatMap { name in
            [
                "\(home)/.local/bin/\(name)",
                "/usr/local/bin/\(name)",
                "/opt/homebrew/bin/\(name)",
            ]
        }
        return searchPaths.contains { FileManager.default.isExecutableFile(atPath: $0) }
    }
}

@MainActor
final class AIProviderRegistry {
    static let shared = AIProviderRegistry()

    private let claudeCodeProvider = ClaudeCodeProvider()
    private let openCodeProvider = OpenCodeProvider()
    private let codexProvider = CodexProvider()
    private let cursorProvider = CursorProvider()
    private let droidProvider = DroidProvider()
    private let piProvider = PiProvider()
    private let grokProvider = GrokProvider()
    private let injectedProviders: [AIProviderIntegration]?
    private let hydrateLoginShellPath: @Sendable () async -> Void
    private let shouldInstallHooksInDebug: @Sendable () -> Bool
    private let hookScriptPath: @Sendable (String, String) -> String?
    private let stageHookResources: @Sendable () -> Bool
    private let installer: HookInstaller
    private var loginShellPathHydration: Task<Void, Never>?
    private var hookResourcesStaged = false
    private var configWatchers: [String: HookConfigWatcher] = [:]

    lazy var providers: [AIProviderIntegration] = injectedProviders ?? [
        claudeCodeProvider,
        openCodeProvider,
        codexProvider,
        cursorProvider,
        droidProvider,
        piProvider,
        grokProvider,
    ]

    init(
        providers: [AIProviderIntegration]? = nil,
        hydrateLoginShellPath: @escaping @Sendable () async -> Void = { await LoginShellPath.hydrate() },
        shouldInstallHooksInDebug: @escaping @Sendable () -> Bool = {
            ProcessInfo.processInfo.environment["FF_AI_HOOKS"] != nil
        },
        hookScriptPath: @escaping @Sendable (String, String) -> String? = {
            MuxyNotificationHooks.scriptPath(named: $0, extension: $1)
        },
        stageHookResources: @escaping @Sendable () -> Bool = {
            MuxyNotificationHooks.stageAll()
        },
        installer: HookInstaller? = nil
    ) {
        injectedProviders = providers
        self.hydrateLoginShellPath = hydrateLoginShellPath
        self.shouldInstallHooksInDebug = shouldInstallHooksInDebug
        self.hookScriptPath = hookScriptPath
        self.stageHookResources = stageHookResources
        self.installer = installer ?? HookInstaller(hookScriptPath: hookScriptPath)
    }

    func prepareForInstallation() {
        _ = stageHookResourcesIfNeeded()
        #if DEBUG
        guard shouldInstallHooksInDebug() else { return }
        #endif
        _ = loginShellPathHydrationTask()
    }

    func installAll() async {
        #if DEBUG
        guard shouldInstallHooksInDebug() else {
            logger.info("Skipping AI hook reconciliation in dev mode (set FF_AI_HOOKS=true to enable)")
            return
        }
        #endif

        let stagingSucceeded = stageHookResourcesIfNeeded()

        guard stagingSucceeded else {
            for provider in providers {
                installer.reconcile(provider, stagingSucceeded: false)
                if !provider.isEnabled {
                    updateConfigWatcher(for: provider)
                }
            }
            return
        }

        let hasDisabledManagedOnly = providers.allSatisfy { !$0.isEnabled }
        if !hasDisabledManagedOnly {
            await loginShellPathHydrationTask().value
        }

        for provider in providers {
            installer.reconcile(provider)
            updateConfigWatcher(for: provider)
        }
    }

    func forceInstall(_ provider: AIProviderIntegration) async {
        guard stageHookResourcesNow() else {
            installer.reconcile(provider, stagingSucceeded: false)
            return
        }
        await loginShellPathHydrationTask().value
        installer.forceReinstall(provider)
        updateConfigWatcher(for: provider)
    }

    func reconcile(_ provider: AIProviderIntegration) {
        installer.reconcile(provider, stagingSucceeded: hookResourcesStaged)
        if hookResourcesStaged || !provider.isEnabled {
            updateConfigWatcher(for: provider)
        }
    }

    private func updateConfigWatcher(for provider: AIProviderIntegration) {
        guard provider.isEnabled else {
            configWatchers.removeValue(forKey: provider.id)
            return
        }
        guard configWatchers[provider.id] == nil else { return }
        let providerID = provider.id
        let watcher = HookConfigWatcher(configPaths: provider.configPaths) { [weak self] in
            Task { @MainActor in
                guard let self, let provider = self.providers.first(where: { $0.id == providerID }) else { return }
                self.reconcileFromWatcher(provider)
            }
        }
        configWatchers[providerID] = watcher
    }

    private func reconcileFromWatcher(_ provider: AIProviderIntegration) {
        let ledger = HookConfigWriteLedger.shared
        guard provider.configPaths.contains(where: { !ledger.isSelfWrite(path: $0) }) else { return }

        if let saturated = provider.configPaths.first(where: { ledger.hasExceededRepairBudget(path: $0) }) {
            let message = "Repeated config rewrites detected for \(saturated) — "
                + "another Muxy build may be managing this config"
            logger.error("\(message)")
            HookHealthStore.shared.noteVerified(providerID: provider.id, state: .conflict(message))
            return
        }

        installer.reconcile(provider, stagingSucceeded: hookResourcesStaged)
    }

    private func loginShellPathHydrationTask() -> Task<Void, Never> {
        if let loginShellPathHydration {
            return loginShellPathHydration
        }
        let hydrateLoginShellPath = hydrateLoginShellPath
        let task = Task.detached(priority: .utility) {
            await hydrateLoginShellPath()
        }
        loginShellPathHydration = task
        return task
    }

    private func stageHookResourcesIfNeeded() -> Bool {
        guard !hookResourcesStaged else { return true }
        return stageHookResourcesNow()
    }

    private func stageHookResourcesNow() -> Bool {
        guard stageHookResources() else {
            hookResourcesStaged = false
            logger.error("Failed to stage AI hook resources")
            return false
        }
        hookResourcesStaged = true
        return true
    }

    func uninstallAll() {
        #if DEBUG
        guard ProcessInfo.processInfo.environment["FF_AI_HOOKS"] != nil else { return }
        #endif

        for provider in providers {
            do {
                try provider.uninstall()
            } catch {
                logger.error("Failed to uninstall \(provider.displayName): \(error.localizedDescription)")
            }
        }
    }

    func notificationSource(for socketType: String) -> MuxyNotification.Source {
        for provider in providers where provider.socketTypeKey == socketType {
            return .aiProvider(provider.id)
        }
        return .socket
    }

    func iconName(for source: MuxyNotification.Source) -> String {
        switch source {
        case .osc:
            "terminal"
        case let .aiProvider(id):
            iconName(forProviderID: id) ?? "sparkles"
        case .socket:
            "network"
        }
    }

    func iconName(forProviderID id: String) -> String? {
        providers.first(where: { $0.id == id })?.iconName
    }

    var agentLaunchProviders: [any AIAgentLaunchProvider] {
        providers.compactMap { $0 as? any AIAgentLaunchProvider }
    }
}
