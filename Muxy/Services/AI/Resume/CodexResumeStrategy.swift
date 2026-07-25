import Foundation

struct CodexResumeStrategy: AgentResumeStrategy {
    func resumeCommand(sessionID: String?, cwd _: String) -> String? {
        guard let sessionID, !sessionID.isEmpty else { return continueLatestCommand }
        return "codex resume \(sessionID)"
    }

    var continueLatestCommand: String? { "codex resume --last" }
}
