import Foundation
import MuxyShared
import Testing

@testable import Muxy

@Suite("AgentStatus")
struct AgentStatusTests {
    @Test("parses a well-formed agent_status message")
    func parsesValidMessage() {
        let paneID = UUID()
        let parsed = NotificationSocketServer.parseAgentStatusMessage("agent_status|claude_hook|\(paneID.uuidString)|working")
        #expect(parsed == NotificationSocketServer.AgentStatusMessage(
            socketType: "claude_hook",
            paneID: paneID,
            status: .working
        ))
    }

    @Test("parses every status value")
    func parsesEveryStatus() {
        let paneID = UUID()
        for status in [AgentStatus.working, .waiting, .idle] {
            let parsed = NotificationSocketServer.parseAgentStatusMessage(
                "agent_status|claude_hook|\(paneID.uuidString)|\(status.rawValue)"
            )
            #expect(parsed?.status == status)
        }
    }

    @Test("parses messages from every provider socket type")
    func parsesEveryProviderSocketType() {
        let paneID = UUID()
        for socketType in ["claude_hook", "cursor_hook", "codex_hook", "droid_hook", "opencode", "pi", "grok_hook"] {
            let parsed = NotificationSocketServer.parseAgentStatusMessage(
                "agent_status|\(socketType)|\(paneID.uuidString)|working"
            )
            #expect(parsed?.socketType == socketType)
            #expect(parsed?.status == .working)
        }
    }

    @Test("rejects an unknown status")
    func rejectsUnknownStatus() {
        let message = "agent_status|claude_hook|\(UUID().uuidString)|busy"
        #expect(NotificationSocketServer.parseAgentStatusMessage(message) == nil)
    }

    @Test("rejects a malformed pane id")
    func rejectsMalformedPaneID() {
        #expect(NotificationSocketServer.parseAgentStatusMessage("agent_status|claude_hook|not-a-uuid|idle") == nil)
    }

    @Test("rejects wrong arity and other heads")
    func rejectsWrongShape() {
        #expect(NotificationSocketServer.parseAgentStatusMessage("agent_status|claude_hook|\(UUID().uuidString)") == nil)
        #expect(NotificationSocketServer.parseAgentStatusMessage("claude_hook|\(UUID().uuidString)|Title|Body") == nil)
        #expect(NotificationSocketServer.parseAgentStatusMessage("agent_status||\(UUID().uuidString)|idle") == nil)
    }

    @Test("parses every normalized lifecycle phase")
    func parsesEveryLifecyclePhase() {
        let paneID = UUID()
        for phase in [AgentLifecyclePhase.working, .waiting, .finished] {
            let parsed = NotificationSocketServer.parseAgentLifecycleMessage(
                "agent_event|codex_hook|\(paneID.uuidString)|\(phase.rawValue)|Codex|Body"
            )
            #expect(parsed == NotificationSocketServer.AgentLifecycleMessage(
                socketType: "codex_hook",
                paneID: paneID,
                phase: phase,
                title: "Codex",
                body: "Body"
            ))
        }
    }

    @Test("lifecycle parser preserves an empty notification and body pipes")
    func lifecycleParserPreservesPayload() {
        let paneID = UUID()
        let parsed = NotificationSocketServer.parseAgentLifecycleMessage(
            "agent_event|opencode|\(paneID.uuidString)|working||part one|part two"
        )
        #expect(parsed?.title == "")
        #expect(parsed?.body == "part one|part two")
    }

    @Test("lifecycle parser rejects malformed messages")
    func rejectsMalformedLifecycleMessages() {
        #expect(NotificationSocketServer.parseAgentLifecycleMessage("agent_event|codex_hook|bad|working||") == nil)
        #expect(NotificationSocketServer.parseAgentLifecycleMessage(
            "agent_event|codex_hook|\(UUID().uuidString)|idle||"
        ) == nil)
        #expect(NotificationSocketServer.parseAgentLifecycleMessage(
            "agent_event||\(UUID().uuidString)|finished||"
        ) == nil)
    }

    @Test("parses a protocol v3 lifecycle event")
    func parsesProtocolV3LifecycleEvent() throws {
        let paneID = UUID()
        let event = AgentHookEventMessage(
            provider: "claude_hook",
            paneID: paneID.uuidString,
            phase: .waiting,
            title: "Claude Code",
            body: "Permission needed",
            pids: [300, 200, 100],
            ts: 1_721_234_567
        )

        let parsed = NotificationSocketServer.parseAgentHookEventMessage(
            try AgentHookWireCodec.encodeEventLine(event)
        )

        #expect(parsed == event)
    }

    @Test("accepts protocol v3 lifecycle events without an explicit pane")
    func acceptsProtocolV3LifecycleEventWithoutPane() throws {
        let event = AgentHookEventMessage(
            provider: "codex_hook",
            paneID: nil,
            phase: .working,
            title: "",
            body: "",
            pids: [300, 200, 100],
            ts: 1_721_234_567
        )

        let parsed = NotificationSocketServer.parseAgentHookEventMessage(
            try AgentHookWireCodec.encodeEventLine(event)
        )

        #expect(parsed == event)
    }

    @Test("rejects invalid protocol v3 lifecycle envelopes")
    func rejectsInvalidProtocolV3LifecycleEnvelopes() throws {
        let valid = AgentHookEventMessage(
            provider: "codex_hook",
            paneID: UUID().uuidString,
            phase: .finished,
            title: "Codex",
            body: "Session completed",
            pids: [],
            ts: 1_721_234_567
        )
        let invalidMessages = [
            AgentHookEventMessage(
                v: 2,
                provider: valid.provider,
                paneID: valid.paneID,
                phase: valid.phase,
                title: valid.title,
                body: valid.body,
                pids: valid.pids,
                ts: valid.ts
            ),
            AgentHookEventMessage(
                kind: "notification",
                provider: valid.provider,
                paneID: valid.paneID,
                phase: valid.phase,
                title: valid.title,
                body: valid.body,
                pids: valid.pids,
                ts: valid.ts
            ),
            AgentHookEventMessage(
                provider: "",
                paneID: valid.paneID,
                phase: valid.phase,
                title: valid.title,
                body: valid.body,
                pids: valid.pids,
                ts: valid.ts
            ),
            AgentHookEventMessage(
                provider: valid.provider,
                paneID: "not-a-uuid",
                phase: valid.phase,
                title: valid.title,
                body: valid.body,
                pids: valid.pids,
                ts: valid.ts
            ),
        ]

        for message in invalidMessages {
            #expect(NotificationSocketServer.parseAgentHookEventMessage(
                try AgentHookWireCodec.encodeEventLine(message)
            ) == nil)
        }
        #expect(NotificationSocketServer.parseAgentHookEventMessage(Data("not-json".utf8)) == nil)
    }

    @Test("only active to idle transitions mark completion")
    func completionTransitions() {
        #expect(AgentStatusStore.marksCompletion(from: .working, to: .idle))
        #expect(AgentStatusStore.marksCompletion(from: .waiting, to: .idle))
        #expect(!AgentStatusStore.marksCompletion(from: nil, to: .idle))
        #expect(!AgentStatusStore.marksCompletion(from: .idle, to: .idle))
        #expect(!AgentStatusStore.marksCompletion(from: .waiting, to: .working))
    }

    @Test("event payload carries the full status context")
    func eventPayloadKeys() {
        let worktreeID = UUID()
        let projectID = UUID()
        let paneID = UUID()
        let payload = AgentStatusStore.eventPayload(
            worktreeID: worktreeID,
            projectID: projectID,
            paneID: paneID,
            providerID: "claude",
            status: .waiting
        )
        #expect(payload["worktreeID"] == worktreeID.uuidString)
        #expect(payload["projectID"] == projectID.uuidString)
        #expect(payload["paneID"] == paneID.uuidString)
        #expect(payload["providerID"] == "claude")
        #expect(payload["status"] == "waiting")
    }

    private func entry(_ status: AgentStatus, worktreeID: UUID, at offset: TimeInterval) -> AgentStatusStore.Entry {
        AgentStatusStore.Entry(
            worktreeID: worktreeID,
            projectID: UUID(),
            paneID: UUID(),
            providerID: "claude",
            status: status,
            updatedAt: Date(timeIntervalSinceReferenceDate: offset)
        )
    }

    @Test("returns nil when no pane contributes to the worktree")
    func aggregateEmpty() {
        #expect(AgentStatusStore.winningEntry(among: []) == nil)
    }

    @Test("the most active pane wins regardless of recency")
    func aggregatePrefersMostActive() {
        let worktreeID = UUID()
        let working = entry(.working, worktreeID: worktreeID, at: 0)
        let waiting = entry(.waiting, worktreeID: worktreeID, at: 100)
        let idle = entry(.idle, worktreeID: worktreeID, at: 200)
        #expect(AgentStatusStore.winningEntry(among: [idle, waiting, working]) == working)
    }

    @Test("ties on status break toward the most recent pane")
    func aggregateBreaksTiesByRecency() {
        let worktreeID = UUID()
        let older = entry(.working, worktreeID: worktreeID, at: 0)
        let newer = entry(.working, worktreeID: worktreeID, at: 100)
        #expect(AgentStatusStore.winningEntry(among: [older, newer]) == newer)
    }
}
