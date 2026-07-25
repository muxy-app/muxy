import Foundation

enum AgentResumeResolver {
    @MainActor
    static func command(
        providerID: String,
        sessionID: String?,
        cwd: String,
        registry: AIProviderRegistry = .shared,
        autoResumeEnabled: Bool = SessionRestorePreferences.autoResumeEnabled()
    ) -> String? {
        guard autoResumeEnabled else { return nil }
        guard let provider = registry.resumeProvider(forProviderID: providerID),
              let strategy = provider.resumeStrategy
        else { return nil }

        if let sessionID, isSafeSessionID(sessionID) {
            return strategy.resumeCommand(sessionID: sessionID, cwd: cwd)
        }
        if let discovered = provider.sessionStore?.sessions(inDirectory: cwd).first,
           isSafeSessionID(discovered.id)
        {
            return strategy.resumeCommand(sessionID: discovered.id, cwd: cwd)
        }
        return strategy.resumeCommand(sessionID: nil, cwd: cwd)
    }

    private static func isSafeSessionID(_ id: String) -> Bool {
        !id.isEmpty && id.allSatisfy { $0.isLetter || $0.isNumber || $0 == "-" || $0 == "_" || $0 == "." }
    }
}
