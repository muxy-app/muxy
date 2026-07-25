import Foundation
import Testing

@testable import Muxy

@Suite("agent.resolveResume verb")
@MainActor
struct MuxyAPIResolveResumeTests {
    @Test("resolves a command for a known provider with explicit id")
    func resolvesCommand() {
        let command = AgentResumeResolver.command(
            providerID: "claude", sessionID: "sid", cwd: "/p")
        #expect(command == "claude --resume sid")
    }

    @Test("agent.session event name is stable")
    func eventName() {
        #expect(ExtensionEventName.agentSession == "agent.session")
    }
}
