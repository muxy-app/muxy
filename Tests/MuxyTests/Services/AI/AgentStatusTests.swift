import Foundation
import MuxyShared
import Testing

@testable import Muxy

@Suite("AgentStatus")
struct AgentStatusTests {
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

    @Test("a repeated event id is delivered once and then suppressed")
    func suppressesRepeatedEventID() {
        var recent = NotificationSocketServer.RecentAgentHookEventIDs()
        let identifier = UUID().uuidString

        let first = recent.registerAndCheckIsFirstDelivery(identifier)
        let second = recent.registerAndCheckIsFirstDelivery(identifier)
        let third = recent.registerAndCheckIsFirstDelivery(identifier)

        #expect(first)
        #expect(!second)
        #expect(!third)
    }

    @Test("distinct event ids are each delivered")
    func deliversDistinctEventIDs() {
        var recent = NotificationSocketServer.RecentAgentHookEventIDs()

        let first = recent.registerAndCheckIsFirstDelivery(UUID().uuidString)
        let second = recent.registerAndCheckIsFirstDelivery(UUID().uuidString)

        #expect(first)
        #expect(second)
    }

    @Test("events without an id are always delivered")
    func alwaysDeliversEventsWithoutID() {
        var recent = NotificationSocketServer.RecentAgentHookEventIDs()

        let firstNil = recent.registerAndCheckIsFirstDelivery(nil)
        let secondNil = recent.registerAndCheckIsFirstDelivery(nil)
        let firstEmpty = recent.registerAndCheckIsFirstDelivery("")
        let secondEmpty = recent.registerAndCheckIsFirstDelivery("")

        #expect(firstNil)
        #expect(secondNil)
        #expect(firstEmpty)
        #expect(secondEmpty)
    }

    @Test("the recent event id window evicts the oldest entries first")
    func evictsOldestEventIDs() {
        var recent = NotificationSocketServer.RecentAgentHookEventIDs()
        let capacity = NotificationSocketServer.RecentAgentHookEventIDs.capacity
        let oldest = "event-0"

        var allAcceptedWhileFilling = true
        for index in 0 ..< capacity where !recent.registerAndCheckIsFirstDelivery("event-\(index)") {
            allAcceptedWhileFilling = false
        }
        let oldestStillRemembered = !recent.registerAndCheckIsFirstDelivery(oldest)
        let overflowAccepted = recent.registerAndCheckIsFirstDelivery("event-\(capacity)")
        let oldestEvicted = recent.registerAndCheckIsFirstDelivery(oldest)

        #expect(allAcceptedWhileFilling)
        #expect(oldestStillRemembered)
        #expect(overflowAccepted)
        #expect(oldestEvicted)
    }

    @Test("event id survives a codec round trip and stays optional")
    func roundTripsEventID() throws {
        let identifier = UUID().uuidString
        let withID = AgentHookEventMessage(
            id: identifier,
            provider: "claude_hook",
            paneID: UUID().uuidString,
            phase: .finished,
            title: "Claude Code",
            body: "Done",
            pids: [],
            ts: 1_721_234_567
        )

        let parsedWithID = try #require(NotificationSocketServer.parseAgentHookEventMessage(
            AgentHookWireCodec.encodeEventLine(withID)
        ))
        #expect(parsedWithID == withID)
        #expect(parsedWithID.id == identifier)

        let withoutID = AgentHookEventMessage(
            provider: "claude_hook",
            paneID: withID.paneID,
            phase: .finished,
            title: withID.title,
            body: withID.body,
            pids: [],
            ts: withID.ts
        )
        let encodedWithoutID = try AgentHookWireCodec.encodeEventLine(withoutID)
        #expect(!String(decoding: encodedWithoutID, as: UTF8.self).contains("\"id\""))
        let parsedWithoutID = try #require(
            NotificationSocketServer.parseAgentHookEventMessage(encodedWithoutID)
        )
        #expect(parsedWithoutID == withoutID)
        #expect(parsedWithoutID.id == nil)
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

@MainActor
private final class ManualGraceScheduler: AgentGraceScheduler {
    private(set) var items: [ManualGraceCancellable] = []
    private(set) var delays: [TimeInterval] = []

    func schedule(after delay: TimeInterval, _ work: @escaping @MainActor () -> Void) -> AgentGraceCancellable {
        let item = ManualGraceCancellable(work: work)
        items.append(item)
        delays.append(delay)
        return item
    }

    var lastDelay: TimeInterval? {
        delays.last
    }

    var pendingCount: Int {
        items.filter { !$0.isCancelled }.count
    }

    func fireLast() {
        items.last?.fire()
    }
}

@MainActor
private final class ManualGraceCancellable: AgentGraceCancellable {
    private let work: () -> Void
    private(set) var isCancelled = false

    init(work: @escaping @MainActor () -> Void) {
        self.work = work
    }

    func cancel() {
        isCancelled = true
    }

    func fire() {
        guard !isCancelled else { return }
        work()
    }
}

@Suite("AgentStatusStore", .serialized)
@MainActor
struct AgentStatusStoreTests {
    private let projectID = UUID()
    private let worktreeID = UUID()

    private func makeContext(
        agentProcessAlive: Bool = false
    ) -> (AgentStatusStore, ManualGraceScheduler, UUID) {
        let (store, scheduler, paneIDs) = makeContext(paneCount: 1, agentProcessAlive: agentProcessAlive)
        return (store, scheduler, paneIDs[0])
    }

    private func makeContext(
        paneCount: Int,
        agentProcessAlive: Bool = false
    ) -> (AgentStatusStore, ManualGraceScheduler, [UUID]) {
        let scheduler = ManualGraceScheduler()
        let store = AgentStatusStore(
            scheduler: scheduler,
            graceDelay: 4,
            waitingGraceDelay: 30,
            isAgentProcessAlive: { _ in agentProcessAlive }
        )
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = TabArea(projectPath: "/tmp/project")
        for _ in 1 ..< paneCount {
            area.createTab()
        }
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = .tabArea(area)
        appState.focusedAreaID[key] = area.id
        let paneIDs = area.tabs.compactMap { $0.content.pane?.id }

        NotificationStore.shared.appState = appState
        NotificationStore.shared.worktreeStore = WorktreeStore(
            persistence: WorktreePersistenceStub(),
            projects: []
        )
        return (store, scheduler, paneIDs)
    }

    private func send(
        _ store: AgentStatusStore,
        _ paneID: UUID,
        _ status: AgentStatus,
        sequence: UInt64
    ) {
        guard let appState = NotificationStore.shared.appState else { return }
        store.update(paneID: paneID, providerID: "claude", status: status, sequence: sequence, appState: appState)
    }

    @Test("drops stale out-of-order events")
    func dropsStaleEvents() {
        let (store, _, paneID) = makeContext()
        send(store, paneID, .working, sequence: 5)
        send(store, paneID, .idle, sequence: 3)
        #expect(store.status(forPane: paneID) == .working)
    }

    @Test("duplicate events are idempotent and mark completion once")
    func duplicateEventsIdempotent() {
        let (store, _, paneID) = makeContext()
        send(store, paneID, .working, sequence: 1)
        send(store, paneID, .idle, sequence: 2)
        #expect(store.isCompletionPending(forPane: paneID))
        store.clearCompletion(for: paneID)
        send(store, paneID, .idle, sequence: 2)
        #expect(!store.isCompletionPending(forPane: paneID))
    }

    @Test("detection loss idles a working agent only after grace with no hook events")
    func detectionLossIdlesAfterGrace() {
        let (store, scheduler, paneID) = makeContext()
        send(store, paneID, .working, sequence: 1)
        store.noteDetectionLost(paneID: paneID)
        #expect(store.status(forPane: paneID) == .working)
        scheduler.fireLast()
        #expect(store.status(forPane: paneID) == .idle)
        #expect(store.isCompletionPending(forPane: paneID))
    }

    @Test("hook event cancels a pending grace transition")
    func hookEventCancelsGrace() {
        let (store, scheduler, paneID) = makeContext()
        send(store, paneID, .working, sequence: 1)
        store.noteDetectionLost(paneID: paneID)
        send(store, paneID, .working, sequence: 2)
        #expect(scheduler.pendingCount == 0)
        scheduler.fireLast()
        #expect(store.status(forPane: paneID) == .working)
    }

    @Test("re-detection cancels a pending grace transition")
    func reDetectionCancelsGrace() {
        let (store, scheduler, paneID) = makeContext()
        send(store, paneID, .working, sequence: 1)
        store.noteDetectionLost(paneID: paneID)
        store.noteDetectionActive(paneID: paneID)
        #expect(scheduler.pendingCount == 0)
        scheduler.fireLast()
        #expect(store.status(forPane: paneID) == .working)
    }

    @Test("a genuinely waiting agent is never idled while its process is alive")
    func waitingNeverIdledWhileProcessAlive() {
        let (store, scheduler, paneID) = makeContext(agentProcessAlive: true)
        send(store, paneID, .waiting, sequence: 1)
        store.noteDetectionLost(paneID: paneID)
        scheduler.fireLast()
        #expect(store.status(forPane: paneID) == .waiting)
    }

    @Test("a killed waiting agent idles after the longer waiting grace")
    func killedWaitingAgentIdlesAfterGrace() {
        let (store, scheduler, paneID) = makeContext(agentProcessAlive: false)
        send(store, paneID, .waiting, sequence: 1)
        store.noteDetectionLost(paneID: paneID)
        #expect(scheduler.lastDelay == 30)
        scheduler.fireLast()
        #expect(store.status(forPane: paneID) == .idle)
        #expect(store.isCompletionPending(forPane: paneID))
    }

    private func send(
        _ store: AgentStatusStore,
        _ paneID: UUID,
        provider: String,
        _ status: AgentStatus,
        sequence: UInt64
    ) {
        guard let appState = NotificationStore.shared.appState else { return }
        store.update(paneID: paneID, providerID: provider, status: status, sequence: sequence, appState: appState)
    }

    @Test("out-of-order events across two providers on one pane keep the newest sequence")
    func outOfOrderAcrossProviders() {
        let (store, _, paneID) = makeContext()
        send(store, paneID, provider: "claude", .working, sequence: 2)
        send(store, paneID, provider: "codex", .waiting, sequence: 4)
        send(store, paneID, provider: "claude", .idle, sequence: 3)
        #expect(store.status(forPane: paneID) == .waiting)
    }

    @Test("an exited agent stops being an active session")
    func endedSessionDropsPresence() {
        let (store, _, paneID) = makeContext()
        send(store, paneID, .working, sequence: 1)
        #expect(store.activeProviderID(forPane: paneID) == "claude")

        store.endSession(paneID: paneID)

        #expect(store.activeProviderID(forPane: paneID) == nil)
        #expect(store.status(forPane: paneID) == .idle)
        #expect(store.isCompletionPending(forPane: paneID))
    }

    @Test("pane identity icon falls back to every active hook provider")
    func paneIdentityIconFallsBackToEveryActiveHookProvider() {
        let (store, _, paneID) = makeContext()

        for (index, provider) in AIProviderRegistry.shared.agentLaunchProviders.enumerated() {
            send(store, paneID, provider: provider.id, .working, sequence: UInt64(index + 1))

            #expect(DetectedAgentStore.shared.agent(for: paneID) == nil)
            #expect(AgentPaneIdentity.iconName(forPane: paneID, agentStatusStore: store) == provider.iconName)
        }
    }

    @Test("detected pane identity wins over active hook provider")
    func detectedPaneIdentityWinsOverActiveHookProvider() {
        let (store, _, paneID) = makeContext()
        send(store, paneID, provider: "codex", .working, sequence: 1)
        DetectedAgentStore.shared.setAgent("claude", for: paneID)
        defer { DetectedAgentStore.shared.resetPane(paneID) }

        #expect(AgentPaneIdentity.iconName(forPane: paneID, agentStatusStore: store) == "claude")
    }

    @Test("pane identity icon prefers detected panes before hook panes")
    func paneIdentityIconPrefersDetectedPanesBeforeHookPanes() {
        let (store, _, paneIDs) = makeContext(paneCount: 2)
        let hookPaneID = paneIDs[0]
        let detectedPaneID = paneIDs[1]
        send(store, hookPaneID, provider: "codex", .working, sequence: 1)
        DetectedAgentStore.shared.setAgent("claude", for: detectedPaneID)
        defer { DetectedAgentStore.shared.resetPane(detectedPaneID) }

        #expect(AgentPaneIdentity.iconName(forPanes: paneIDs, agentStatusStore: store) == "claude")
    }

    @Test("ending a session cancels a pending grace transition")
    func endSessionCancelsGrace() {
        let (store, scheduler, paneID) = makeContext()
        send(store, paneID, .working, sequence: 1)
        store.noteDetectionLost(paneID: paneID)

        store.endSession(paneID: paneID)

        #expect(scheduler.pendingCount == 0)
    }

    @Test("a late session-end event does not revive an ended session")
    func lateFinishedEventKeepsSessionEnded() {
        let (store, _, paneID) = makeContext()
        send(store, paneID, .working, sequence: 1)
        store.endSession(paneID: paneID)

        send(store, paneID, .idle, sequence: 2)

        #expect(store.activeProviderID(forPane: paneID) == nil)
    }

    @Test("a late session-end event without earlier events stays hidden")
    func lateFinishedEventWithoutHistoryStaysHidden() {
        let (store, _, paneID) = makeContext()
        store.endSession(paneID: paneID)

        send(store, paneID, .idle, sequence: 1)

        #expect(store.activeProviderID(forPane: paneID) == nil)
    }

    @Test("a new agent run restores the session")
    func newRunRestoresPresence() {
        let (store, _, paneID) = makeContext()
        send(store, paneID, .working, sequence: 1)
        store.endSession(paneID: paneID)

        send(store, paneID, .working, sequence: 2)

        #expect(store.activeProviderID(forPane: paneID) == "claude")
    }

    @Test("re-detecting an agent restores the session")
    func reDetectionRestoresPresence() {
        let (store, _, paneID) = makeContext()
        send(store, paneID, .working, sequence: 1)
        store.endSession(paneID: paneID)

        store.noteDetectionActive(paneID: paneID)

        #expect(store.activeProviderID(forPane: paneID) == "claude")
    }

    @Test("command exit keeps a reusable pane ended until another agent starts")
    func reusablePaneCommandExitKeepsSessionEnded() {
        let (store, _, paneID) = makeContext()
        send(store, paneID, .working, sequence: 1)

        store.commandExited(paneID: paneID, closesPane: false)
        send(store, paneID, .idle, sequence: 2)

        #expect(store.activeProviderID(forPane: paneID) == nil)
        #expect(store.status(forPane: paneID) == .idle)
        #expect(store.isCompletionPending(forPane: paneID))

        send(store, paneID, .working, sequence: 3)

        #expect(store.activeProviderID(forPane: paneID) == "claude")
        #expect(store.status(forPane: paneID) == .working)
        #expect(!store.isCompletionPending(forPane: paneID))
    }

    @Test("command exit clears state when its pane closes")
    func closingPaneCommandExitRemovesSession() {
        let (store, _, paneID) = makeContext()
        send(store, paneID, .working, sequence: 1)

        store.commandExited(paneID: paneID, closesPane: true)

        #expect(store.status(forPane: paneID) == nil)
        #expect(store.activeProviderID(forPane: paneID) == nil)
        #expect(!store.isCompletionPending(forPane: paneID))
    }

    @Test("pane close drops all session state")
    func paneCloseDropsSessions() {
        let (store, _, paneID) = makeContext()
        send(store, paneID, .working, sequence: 1)
        store.removePane(paneID)
        #expect(store.status(forPane: paneID) == nil)
        #expect(!store.isCompletionPending(forPane: paneID))
        send(store, paneID, .idle, sequence: 1)
        #expect(store.status(forPane: paneID) == .idle)
    }
}

private final class WorkspacePersistenceStub: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
}

@MainActor
private final class SelectionStoreStub: ActiveProjectSelectionStoring {
    private var activeProjectID: UUID?
    private var activeWorktreeIDs: [UUID: UUID] = [:]
    func loadActiveProjectID() -> UUID? { activeProjectID }
    func saveActiveProjectID(_ id: UUID?) { activeProjectID = id }
    func loadActiveWorktreeIDs() -> [UUID: UUID] { activeWorktreeIDs }
    func saveActiveWorktreeIDs(_ ids: [UUID: UUID]) { activeWorktreeIDs = ids }
}

@MainActor
private final class TerminalViewRemovingStub: TerminalViewRemoving {
    func removeView(for _: UUID) {}
    func needsConfirmQuit(for _: UUID) -> Bool { false }
}

private final class WorktreePersistenceStub: WorktreePersisting {
    func loadWorktrees(projectID _: UUID) throws -> [Worktree] { [] }
    func saveWorktrees(_: [Worktree], projectID _: UUID) throws {}
    func removeWorktrees(projectID _: UUID) throws {}
}
