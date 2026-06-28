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
    func install(hookScriptPath: String) throws
    func uninstall() throws
}

extension AIProviderIntegration {
    func isHookInstalled() -> Bool {
        false
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
        }
    ) {
        injectedProviders = providers
        self.hydrateLoginShellPath = hydrateLoginShellPath
        self.shouldInstallHooksInDebug = shouldInstallHooksInDebug
    }

    func prepareForInstallation() {
        _ = loginShellPathHydrationTask()
    }

    func installAll() async {
        #if DEBUG
        guard shouldInstallHooksInDebug() else {
            logger.info("Skipping AI hooks install in dev mode (set FF_AI_HOOKS=true to enable)")
            await refreshInstalledHooks()
            return
        }
        #endif

        for provider in providers {
            guard provider.isEnabled else {
                logger.info("\(provider.displayName) is disabled, uninstalling hook if present")
                do {
                    try provider.uninstall()
                } catch {
                    logger.warning("Failed to uninstall \(provider.displayName): \(error.localizedDescription)")
                }
                continue
            }
            await loginShellPathHydrationTask().value

            guard provider.isToolInstalled() else {
                logger.info("\(provider.displayName) tool not installed, skipping hook install")
                continue
            }
            guard let hookScript = MuxyNotificationHooks
                .scriptPath(named: provider.hookScriptName, extension: provider.hookScriptExtension)
            else {
                logger.warning("Hook script \(provider.hookScriptName) not found in bundle, skipping \(provider.displayName)")
                continue
            }
            do {
                try provider.install(hookScriptPath: hookScript)
                logger.info("Installed \(provider.displayName) integration")
            } catch {
                logger.error("Failed to install \(provider.displayName): \(error.localizedDescription)")
            }
        }
    }

    private func refreshInstalledHooks() async {
        for provider in providers where provider.isEnabled && provider.isHookInstalled() {
            guard let hookScript = MuxyNotificationHooks
                .scriptPath(named: provider.hookScriptName, extension: provider.hookScriptExtension)
            else { continue }
            do {
                try provider.install(hookScriptPath: hookScript)
                logger.info("Refreshed \(provider.displayName) hook to the bundled version")
            } catch {
                logger.warning("Failed to refresh \(provider.displayName) hook: \(error.localizedDescription)")
            }
        }
    }

    func forceInstall(_ provider: AIProviderIntegration) async {
        guard let hookScript = MuxyNotificationHooks.scriptPath(named: provider.hookScriptName, extension: provider.hookScriptExtension)
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
        if let loginShellPathHydration { return loginShellPathHydration }
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
}
