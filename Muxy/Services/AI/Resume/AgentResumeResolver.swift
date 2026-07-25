import Foundation

enum AgentResumeResolver {
    @MainActor
    static func command(
        providerID: String,
        sessionID: String?,
        cwd: String,
        registry: AIProviderRegistry = .shared
    ) -> String? {
        guard let provider = registry.resumeProvider(forProviderID: providerID),
              let strategy = provider.resumeStrategy
        else { return nil }

        if let sessionID, !sessionID.isEmpty {
            return strategy.resumeCommand(sessionID: sessionID, cwd: cwd)
        }
        if let discovered = provider.sessionStore?.sessions(inDirectory: cwd).first {
            return strategy.resumeCommand(sessionID: discovered.id, cwd: cwd)
        }
        return strategy.resumeCommand(sessionID: nil, cwd: cwd)
    }
}
