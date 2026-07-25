import Foundation
import Testing

@testable import Muxy

@Suite("AgentResumeResolver")
@MainActor
struct AgentResumeResolverTests {
    private struct StubStrategy: AgentResumeStrategy {
        func resumeCommand(sessionID: String?, cwd _: String) -> String? {
            guard let sessionID else { return "tool --continue" }
            return "tool --resume \(sessionID)"
        }
        var continueLatestCommand: String? { "tool --continue" }
    }

    private struct StubStore: AgentSessionStore {
        let refs: [AgentSessionRef]
        func sessions(inDirectory _: String) -> [AgentSessionRef] { refs }
    }

    private struct StubProvider: AIProviderIntegration, AgentResumeProviding {
        let id = "tool"
        let displayName = "Tool"
        let socketTypeKey = "tool"
        let iconName = "x"
        let executableNames = ["tool"]
        let store: [AgentSessionRef]
        var resumeStrategy: AgentResumeStrategy? { StubStrategy() }
        var sessionStore: AgentSessionStore? { StubStore(refs: store) }
        func install(hookScriptPath: String) throws {}
        func uninstall() throws {}
    }

    private func registry(store: [AgentSessionRef]) -> AIProviderRegistry {
        AIProviderRegistry(providers: [StubProvider(store: store)])
    }

    @Test("explicit session id wins")
    func explicitID() {
        let command = AgentResumeResolver.command(
            providerID: "tool", sessionID: "abc", cwd: "/p", registry: registry(store: []))
        #expect(command == "tool --resume abc")
    }

    @Test("falls back to newest discovered session")
    func discovered() {
        let ref = AgentSessionRef(id: "disc", providerID: "tool", cwd: "/p", gitBranch: nil,
            title: nil, preview: nil, updatedAt: Date(timeIntervalSince1970: 5), pinned: false, archived: false)
        let command = AgentResumeResolver.command(
            providerID: "tool", sessionID: nil, cwd: "/p", registry: registry(store: [ref]))
        #expect(command == "tool --resume disc")
    }

    @Test("falls back to continue-latest when nothing discovered")
    func continueLatest() {
        let command = AgentResumeResolver.command(
            providerID: "tool", sessionID: nil, cwd: "/p", registry: registry(store: []))
        #expect(command == "tool --continue")
    }

    @Test("unknown provider yields nil")
    func unknown() {
        let command = AgentResumeResolver.command(
            providerID: "ghost", sessionID: nil, cwd: "/p", registry: registry(store: []))
        #expect(command == nil)
    }
}
