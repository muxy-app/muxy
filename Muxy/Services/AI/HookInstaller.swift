import Foundation
import os

private let installerLogger = Logger(subsystem: "app.muxy", category: "HookInstaller")

@MainActor
final class HookInstaller {
    enum Outcome: Equatable {
        case healthy
        case repaired
        case cliMissing
        case conflict(String)
        case failed(String)
        case skippedDisabled
    }

    private let hookScriptPath: @Sendable (String, String) -> String?
    private let stagedFileExists: @Sendable (String) -> Bool
    private let stagedFileExecutable: @Sendable (String) -> Bool
    private let health: HookHealthStore

    init(
        hookScriptPath: @escaping @Sendable (String, String) -> String? = {
            MuxyNotificationHooks.scriptPath(named: $0, extension: $1)
        },
        stagedFileExists: @escaping @Sendable (String) -> Bool = {
            FileManager.default.fileExists(atPath: $0)
        },
        stagedFileExecutable: @escaping @Sendable (String) -> Bool = {
            FileManager.default.isExecutableFile(atPath: $0)
        },
        health: HookHealthStore = .shared
    ) {
        self.hookScriptPath = hookScriptPath
        self.stagedFileExists = stagedFileExists
        self.stagedFileExecutable = stagedFileExecutable
        self.health = health
    }

    @discardableResult
    func reconcile(_ provider: AIProviderIntegration) -> Outcome {
        guard provider.isEnabled else {
            removeManagedState(provider)
            return .skippedDisabled
        }

        guard let scriptPath = stagedScriptPath(for: provider) else {
            let message = "Staged hook \(provider.hookScriptName) not found"
            health.noteVerified(providerID: provider.id, state: .error(message))
            return .failed(message)
        }

        guard stagedResourcesReady(for: provider, scriptPath: scriptPath) else {
            let message = "Staged hook resource is missing or not executable"
            health.noteVerified(providerID: provider.id, state: .error(message))
            return .failed(message)
        }

        return apply(provider, scriptPath: scriptPath)
    }

    @discardableResult
    func forceReinstall(_ provider: AIProviderIntegration) -> Outcome {
        guard let scriptPath = stagedScriptPath(for: provider) else {
            let message = "Staged hook \(provider.hookScriptName) not found"
            health.noteVerified(providerID: provider.id, state: .error(message))
            return .failed(message)
        }
        do {
            try provider.uninstall()
            try provider.install(hookScriptPath: scriptPath)
        } catch let error as CodexProviderError {
            let message = error.localizedDescription
            health.noteRepaired(providerID: provider.id, state: .conflict(message))
            return .conflict(message)
        } catch {
            let message = error.localizedDescription
            health.noteRepaired(providerID: provider.id, state: .error(message))
            return .failed(message)
        }
        return finishVerification(provider, scriptPath: scriptPath, repaired: true)
    }

    private func apply(_ provider: AIProviderIntegration, scriptPath: String) -> Outcome {
        switch provider.verify(hookScriptPath: scriptPath) {
        case .satisfied:
            health.noteVerified(providerID: provider.id, state: .installed)
            return .healthy
        case let .conflict(message):
            health.noteVerified(providerID: provider.id, state: .conflict(message))
            return .conflict(message)
        case let .failed(message):
            health.noteVerified(providerID: provider.id, state: .error(message))
            return .failed(message)
        case .needsRepair:
            return repair(provider, scriptPath: scriptPath)
        }
    }

    private func repair(_ provider: AIProviderIntegration, scriptPath: String) -> Outcome {
        if !provider.hasManagedState(), !provider.isToolInstalled() {
            health.noteVerified(providerID: provider.id, state: .cliMissing)
            return .cliMissing
        }

        do {
            try provider.install(hookScriptPath: scriptPath)
        } catch let error as CodexProviderError {
            let message = error.localizedDescription
            health.noteRepaired(providerID: provider.id, state: .conflict(message))
            return .conflict(message)
        } catch {
            let message = error.localizedDescription
            health.noteRepaired(providerID: provider.id, state: .error(message))
            return .failed(message)
        }
        return finishVerification(provider, scriptPath: scriptPath, repaired: true)
    }

    private func finishVerification(
        _ provider: AIProviderIntegration,
        scriptPath: String,
        repaired: Bool
    ) -> Outcome {
        switch provider.verify(hookScriptPath: scriptPath) {
        case .satisfied:
            health.noteRepaired(providerID: provider.id, state: .installed)
            return repaired ? .repaired : .healthy
        case let .conflict(message):
            health.noteRepaired(providerID: provider.id, state: .conflict(message))
            return .conflict(message)
        case .needsRepair,
             .failed:
            let message = "Hook could not be verified after repair"
            health.noteRepaired(providerID: provider.id, state: .error(message))
            return .failed(message)
        }
    }

    private func removeManagedState(_ provider: AIProviderIntegration) {
        defer { health.reset(providerID: provider.id) }
        guard provider.hasManagedState() else { return }
        do {
            try provider.uninstall()
        } catch {
            installerLogger.warning("Failed to uninstall \(provider.displayName): \(error.localizedDescription)")
        }
    }

    private func stagedScriptPath(for provider: AIProviderIntegration) -> String? {
        hookScriptPath(provider.hookScriptName, provider.hookScriptExtension)
    }

    private func stagedResourcesReady(for provider: AIProviderIntegration, scriptPath: String) -> Bool {
        guard stagedFileExists(scriptPath) else { return false }
        guard provider.hookScriptExtension == "sh" else { return true }
        return stagedFileExecutable(scriptPath)
    }
}
