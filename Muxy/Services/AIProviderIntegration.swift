import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "AIProviderRegistry")

protocol AIProviderIntegration {
    var id: String { get }
    var displayName: String { get }
    var socketTypeKey: String { get }
    var iconName: String { get }
    var executableNames: [String] { get }

    func isToolInstalled() -> Bool
    func install(hookScriptPath: String) throws
    func uninstall() throws
}

extension AIProviderIntegration {
    var settingsKey: String { "muxy.notifications.provider.\(id).enabled" }

    var isEnabled: Bool {
        get { UserDefaults.standard.bool(forKey: settingsKey, fallback: true) }
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

    lazy var providers: [AIProviderIntegration] = [
        claudeCodeProvider,
        openCodeProvider,
    ]

    lazy var usageProviders: [any AIUsageProvider] = [
        claudeCodeProvider,
        CodexUsageProvider(),
        CopilotUsageProvider(),
        CursorUsageProvider(),
        AmpUsageProvider(),
        ZaiUsageProvider(),
        MiniMaxUsageProvider(),
        KimiUsageProvider(),
        FactoryUsageProvider(),
    ]

    private init() {}

    func installAll() {
        #if DEBUG
        guard ProcessInfo.processInfo.environment["FF_AI_HOOKS"] != nil else {
            logger.info("Skipping AI hooks install in dev mode (set FF_AI_HOOKS=true to enable)")
            return
        }
        #endif

        guard let hookScript = MuxyNotificationHooks.hookScriptPath else {
            logger.info("Hook script not found, skipping AI provider installs")
            return
        }

        for provider in providers {
            guard provider.isEnabled else {
                try? provider.uninstall()
                continue
            }
            guard provider.isToolInstalled() else { continue }
            do {
                try provider.install(hookScriptPath: hookScript)
                logger.info("Installed \(provider.displayName) integration")
            } catch {
                logger.error("Failed to install \(provider.displayName): \(error.localizedDescription)")
            }
        }
    }

    func forceInstall(_ provider: AIProviderIntegration) {
        guard let hookScript = MuxyNotificationHooks.hookScriptPath else {
            logger.info("Hook script not found, skipping force install")
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
        case .osc: "terminal"
        case let .aiProvider(id):
            providers.first { $0.id == id }?.iconName ?? "sparkles"
        case .socket: "network"
        }
    }
}
