import Foundation
import Testing

@testable import Muxy

@Suite("TerminalTabSnapshot agentSession")
struct TerminalTabSnapshotAgentSessionTests {
    @Test("round-trips an agent session")
    func roundTrips() throws {
        let snapshot = TerminalTabSnapshot(
            kind: .terminal, customTitle: nil, colorID: nil, isPinned: false,
            projectPath: "/p", paneTitle: "T",
            agentSession: AgentSessionSnapshot(providerID: "claude", sessionID: "sid", cwd: "/p"))
        let data = try JSONEncoder().encode(snapshot)
        let decoded = try JSONDecoder().decode(TerminalTabSnapshot.self, from: data)
        #expect(decoded.agentSession?.providerID == "claude")
        #expect(decoded.agentSession?.sessionID == "sid")
    }

    @Test("legacy snapshot without agentSession still decodes")
    func legacyDecodes() throws {
        let json = #"{"kind":"terminal","id":"\#(UUID().uuidString)","isPinned":false,"projectPath":"/p","paneTitle":"T"}"#
        let decoded = try JSONDecoder().decode(TerminalTabSnapshot.self, from: Data(json.utf8))
        #expect(decoded.agentSession == nil)
    }
}
