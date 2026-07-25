import Foundation

struct AgyProvider: AIProviderIntegration, AgentResumeProviding {
    let id = "agy"
    let displayName = "Antigravity"
    let socketTypeKey = "agy_hook"
    let iconName = "sparkles"
    let executableNames = ["agy"]

    var resumeStrategy: AgentResumeStrategy? { AgyResumeStrategy() }
    var sessionStore: AgentSessionStore? { AgySessionStore() }

    func install(hookScriptPath: String) throws {}
    func uninstall() throws {}
}
