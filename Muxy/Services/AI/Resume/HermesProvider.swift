import Foundation

struct HermesProvider: AIProviderIntegration, AgentResumeProviding {
    let id = "hermes"
    let displayName = "Hermes"
    let socketTypeKey = "hermes_hook"
    let iconName = "bolt"
    let executableNames = ["hermes"]

    var resumeStrategy: AgentResumeStrategy? { HermesResumeStrategy() }
    var sessionStore: AgentSessionStore? { HermesSessionStore() }

    func install(hookScriptPath: String) throws {}
    func uninstall() throws {}
}
