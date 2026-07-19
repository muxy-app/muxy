import Foundation
import Testing

@testable import Muxy

@Suite("TerminalEnvVarBuilder")
@MainActor
struct TerminalEnvVarBuilderTests {
    @Test("terminals advertise the staged hook binary and pane identity")
    func advertisesHookEnvironment() {
        let paneID = UUID()
        let key = WorktreeKey(projectID: UUID(), worktreeID: UUID())

        let environment = Dictionary(
            uniqueKeysWithValues: TerminalEnvVarBuilder.build(paneID: paneID, worktreeKey: key)
                .map { ($0.key, $0.value) }
        )

        #expect(environment["MUXY_AGENT_EVENT_PROTOCOL"] == nil)
        #expect(environment["MUXY_PANE_ID"] == paneID.uuidString)
        #expect(environment["MUXY_HOOK_BIN"] == MuxyNotificationHooks.hookBinaryPath)
        #expect(
            environment["MUXY_HOOK_SCRIPT"]
                == MuxyNotificationHooks.stagedScriptPath(named: "muxy-claude-hook", extension: "sh")
        )
    }
}
