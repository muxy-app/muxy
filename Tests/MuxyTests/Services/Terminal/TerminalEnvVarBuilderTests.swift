import Foundation
import Testing

@testable import Muxy

@Suite("TerminalEnvVarBuilder")
@MainActor
struct TerminalEnvVarBuilderTests {
    @Test("terminals advertise the normalized agent lifecycle protocol")
    func advertisesAgentLifecycleProtocol() {
        let paneID = UUID()
        let key = WorktreeKey(projectID: UUID(), worktreeID: UUID())

        let environment = Dictionary(
            uniqueKeysWithValues: TerminalEnvVarBuilder.build(paneID: paneID, worktreeKey: key)
                .map { ($0.key, $0.value) }
        )

        #expect(environment["MUXY_AGENT_EVENT_PROTOCOL"] == "2")
        #expect(environment["MUXY_PANE_ID"] == paneID.uuidString)
    }
}
