import Foundation

struct CursorResumeStrategy: AgentResumeStrategy {
    func resumeCommand(sessionID: String?, cwd _: String) -> String? {
        guard let sessionID, !sessionID.isEmpty else { return continueLatestCommand }
        return "cursor-agent --resume=\(sessionID)"
    }

    var continueLatestCommand: String? { nil }
}
