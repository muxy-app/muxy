import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "AIProviderRegistry")

protocol AIProviderIntegration {
    var id: String { get }
    var displayName: String { get }
    var socketTypeKey: String { get }
    var iconName: String { get }
    var executableNames: [String] { get }
    var hookScriptName: String { get }
    var hookScriptExtension: String { get }

    func isToolInstalled() -> Bool
    func isHookInstalled() -> Bool
    func hasManagedState() -> Bool
    func install(hookScriptPath: String) throws
    func uninstall() throws
}

extension AIProviderIntegration {
    func isHookInstalled() -> Bool {
        false
    }

    func hasManagedState() -> Bool {
        isHookInstalled()
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
    private var loginShellPathHydration: Task<Void, Never>?

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
        }
    ) {
        injectedProviders = providers
        self.hydrateLoginShellPath = hydrateLoginShellPath
        self.shouldInstallHooksInDebug = shouldInstallHooksInDebug
        self.hookScriptPath = hookScriptPath
    }

    func prepareForInstallation() {
        #if DEBUG
        guard shouldInstallHooksInDebug() else { return }
        #endif
        _ = loginShellPathHydrationTask()
    }

    func installAll() async {
        #if DEBUG
        let installMissingHooks = shouldInstallHooksInDebug()
        if !installMissingHooks {
            logger.info("Reconciling installed AI hooks in dev mode (set FF_AI_HOOKS=true to install missing hooks)")
        }
        #else
        let installMissingHooks = true
        #endif

        for provider in providers {
            guard provider.isEnabled else {
                removeInstalledHook(for: provider)
                continue
            }

            if provider.isHookInstalled() {
                installHook(for: provider, action: "Refreshed")
                continue
            }

            guard installMissingHooks else { continue }
            await loginShellPathHydrationTask().value

            guard provider.isToolInstalled() else {
                logger.info("\(provider.displayName) tool not installed, skipping hook install")
                continue
            }

            installHook(for: provider, action: "Installed")
        }
    }

    private func removeInstalledHook(for provider: AIProviderIntegration) {
        guard provider.hasManagedState() else { return }
        logger.info("\(provider.displayName) is disabled, removing managed hook state")
        do {
            try provider.uninstall()
        } catch {
            logger.warning("Failed to uninstall \(provider.displayName): \(error.localizedDescription)")
        }
    }

    private func installHook(for provider: AIProviderIntegration, action: String) {
        guard let hookScript = hookScriptPath(provider.hookScriptName, provider.hookScriptExtension)
        else {
            logger.warning("Hook script \(provider.hookScriptName) not found in bundle, skipping \(provider.displayName)")
            return
        }
        do {
            try provider.install(hookScriptPath: hookScript)
            logger.info("\(action) \(provider.displayName) integration")
        } catch {
            logger.error("Failed to reconcile \(provider.displayName): \(error.localizedDescription)")
        }
    }

    func forceInstall(_ provider: AIProviderIntegration) async {
        guard let hookScript = hookScriptPath(provider.hookScriptName, provider.hookScriptExtension)
        else {
            logger.warning("Hook script \(provider.hookScriptName) not found, cannot force-install \(provider.displayName)")
            return
        }

        do {
            try provider.uninstall()
            try provider.install(hookScriptPath: hookScript)
            logger.info("Force-installed \(provider.displayName) integration")
        } catch {
            logger.error("Failed to force-install \(provider.displayName): \(error.localizedDescription)")
        }
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
