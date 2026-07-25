import Foundation

struct ClaudeResumeStrategy: AgentResumeStrategy {
    func resumeCommand(sessionID: String?, cwd _: String) -> String? {
        guard let sessionID, !sessionID.isEmpty else { return continueLatestCommand }
        return "claude --resume \(sessionID)"
    }

    var continueLatestCommand: String? { "claude --continue" }
}
