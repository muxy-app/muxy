import Foundation

struct AgyResumeStrategy: AgentResumeStrategy {
    func resumeCommand(sessionID: String?, cwd _: String) -> String? {
        guard let sessionID, !sessionID.isEmpty else { return continueLatestCommand }
        return "agy --conversation \(sessionID)"
    }

    var continueLatestCommand: String? { nil }
}
