import Foundation

struct HermesResumeStrategy: AgentResumeStrategy {
    func resumeCommand(sessionID: String?, cwd _: String) -> String? {
        guard let sessionID, !sessionID.isEmpty else { return continueLatestCommand }
        return "hermes --resume \(sessionID)"
    }

    var continueLatestCommand: String? { "hermes --continue" }
}
