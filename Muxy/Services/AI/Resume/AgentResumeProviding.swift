import Foundation

protocol AgentResumeStrategy {
    func resumeCommand(sessionID: String?, cwd: String) -> String?
    var continueLatestCommand: String? { get }
}

protocol AgentSessionStore {
    func sessions(inDirectory directory: String) -> [AgentSessionRef]
}

protocol AgentResumeProviding {
    var resumeStrategy: AgentResumeStrategy? { get }
    var sessionStore: AgentSessionStore? { get }
}

extension AgentResumeProviding {
    var resumeStrategy: AgentResumeStrategy? { nil }
    var sessionStore: AgentSessionStore? { nil }
}
