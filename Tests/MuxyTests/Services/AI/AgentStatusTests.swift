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

    func schedule(after _: TimeInterval, _ work: @escaping @MainActor () -> Void) -> AgentGraceCancellable {
        let item = ManualGraceCancellable(work: work)
        items.append(item)
        return item
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

    private func makeContext() -> (AgentStatusStore, ManualGraceScheduler, UUID) {
        let scheduler = ManualGraceScheduler()
        let store = AgentStatusStore(scheduler: scheduler, graceDelay: 4)
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = TabArea(projectPath: "/tmp/project")
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = .tabArea(area)
        appState.focusedAreaID[key] = area.id
        let paneID = area.tabs.last!.content.pane!.id

        NotificationStore.shared.appState = appState
        NotificationStore.shared.worktreeStore = WorktreeStore(
            persistence: WorktreePersistenceStub(),
            projects: []
        )
        return (store, scheduler, paneID)
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
    func dropsStaleEvents() async {
        await SharedNotificationStateGate.run {
            let (store, _, paneID) = makeContext()
            send(store, paneID, .working, sequence: 5)
            send(store, paneID, .idle, sequence: 3)
            #expect(store.status(forPane: paneID) == .working)
        }
    }

    @Test("applies newer events after older ones")
    func appliesNewerEvents() async {
        await SharedNotificationStateGate.run {
            let (store, _, paneID) = makeContext()
            send(store, paneID, .working, sequence: 1)
            send(store, paneID, .waiting, sequence: 2)
            #expect(store.status(forPane: paneID) == .waiting)
        }
    }

    @Test("duplicate events are idempotent and mark completion once")
    func duplicateEventsIdempotent() async {
        await SharedNotificationStateGate.run {
            let (store, _, paneID) = makeContext()
            send(store, paneID, .working, sequence: 1)
            send(store, paneID, .idle, sequence: 2)
            #expect(store.isCompletionPending(forPane: paneID))
            store.clearCompletion(for: paneID)
            send(store, paneID, .idle, sequence: 2)
            #expect(!store.isCompletionPending(forPane: paneID))
        }
    }

    @Test("completion badge fires exactly once per finish")
    func completionFiresOncePerFinish() async {
        await SharedNotificationStateGate.run {
            let (store, _, paneID) = makeContext()
            send(store, paneID, .working, sequence: 1)
            send(store, paneID, .idle, sequence: 2)
            #expect(store.isCompletionPending(forPane: paneID))
            store.clearCompletion(for: paneID)
            send(store, paneID, .idle, sequence: 3)
            #expect(!store.isCompletionPending(forPane: paneID))
            send(store, paneID, .working, sequence: 4)
            send(store, paneID, .idle, sequence: 5)
            #expect(store.isCompletionPending(forPane: paneID))
        }
    }

    @Test("detection loss idles a working agent only after grace with no hook events")
    func detectionLossIdlesAfterGrace() async {
        await SharedNotificationStateGate.run {
            let (store, scheduler, paneID) = makeContext()
            send(store, paneID, .working, sequence: 1)
            store.noteDetectionLost(paneID: paneID)
            #expect(store.status(forPane: paneID) == .working)
            scheduler.fireLast()
            #expect(store.status(forPane: paneID) == .idle)
            #expect(store.isCompletionPending(forPane: paneID))
        }
    }

    @Test("hook event cancels a pending grace transition")
    func hookEventCancelsGrace() async {
        await SharedNotificationStateGate.run {
            let (store, scheduler, paneID) = makeContext()
            send(store, paneID, .working, sequence: 1)
            store.noteDetectionLost(paneID: paneID)
            send(store, paneID, .working, sequence: 2)
            #expect(scheduler.pendingCount == 0)
            scheduler.fireLast()
            #expect(store.status(forPane: paneID) == .working)
        }
    }

    @Test("re-detection cancels a pending grace transition")
    func reDetectionCancelsGrace() async {
        await SharedNotificationStateGate.run {
            let (store, scheduler, paneID) = makeContext()
            send(store, paneID, .working, sequence: 1)
            store.noteDetectionLost(paneID: paneID)
            store.noteDetectionActive(paneID: paneID)
            #expect(scheduler.pendingCount == 0)
            scheduler.fireLast()
            #expect(store.status(forPane: paneID) == .working)
        }
    }

    @Test("a waiting agent is never idled by detection loss")
    func waitingNeverIdledByDetectionLoss() async {
        await SharedNotificationStateGate.run {
            let (store, scheduler, paneID) = makeContext()
            send(store, paneID, .waiting, sequence: 1)
            store.noteDetectionLost(paneID: paneID)
            #expect(scheduler.pendingCount == 0)
            #expect(store.status(forPane: paneID) == .waiting)
        }
    }

    @Test("grace does not idle when a hook event arrives before it fires")
    func graceSkippedWhenHookArrives() async {
        await SharedNotificationStateGate.run {
            let (store, scheduler, paneID) = makeContext()
            send(store, paneID, .working, sequence: 1)
            store.noteDetectionLost(paneID: paneID)
            send(store, paneID, .waiting, sequence: 2)
            scheduler.fireLast()
            #expect(store.status(forPane: paneID) == .waiting)
        }
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
    func outOfOrderAcrossProviders() async {
        await SharedNotificationStateGate.run {
            let (store, _, paneID) = makeContext()
            send(store, paneID, provider: "claude", .working, sequence: 2)
            send(store, paneID, provider: "codex", .waiting, sequence: 4)
            send(store, paneID, provider: "claude", .idle, sequence: 3)
            #expect(store.status(forPane: paneID) == .waiting)
        }
    }

    @Test("a newer provider event supersedes an older one regardless of arrival order")
    func newerProviderSupersedesOlder() async {
        await SharedNotificationStateGate.run {
            let (store, _, paneID) = makeContext()
            send(store, paneID, provider: "codex", .idle, sequence: 5)
            send(store, paneID, provider: "claude", .working, sequence: 2)
            #expect(store.status(forPane: paneID) == .idle)
        }
    }

    @Test("re-detection after loss keeps a working agent alive through the grace window")
    func reDetectionKeepsWorkingThroughGrace() async {
        await SharedNotificationStateGate.run {
            let (store, scheduler, paneID) = makeContext()
            send(store, paneID, .working, sequence: 1)
            store.noteDetectionLost(paneID: paneID)
            store.noteDetectionActive(paneID: paneID)
            store.noteDetectionLost(paneID: paneID)
            store.noteDetectionActive(paneID: paneID)
            scheduler.fireLast()
            #expect(store.status(forPane: paneID) == .working)
        }
    }

    @Test("a waiting hook event during the grace window blocks the idle transition")
    func waitingHookDuringGraceBlocksIdle() async {
        await SharedNotificationStateGate.run {
            let (store, scheduler, paneID) = makeContext()
            send(store, paneID, .working, sequence: 1)
            store.noteDetectionLost(paneID: paneID)
            send(store, paneID, .waiting, sequence: 2)
            scheduler.fireLast()
            #expect(store.status(forPane: paneID) == .waiting)
            #expect(!store.isCompletionPending(forPane: paneID))
        }
    }

    @Test("pane close drops all session state")
    func paneCloseDropsSessions() async {
        await SharedNotificationStateGate.run {
            let (store, _, paneID) = makeContext()
            send(store, paneID, .working, sequence: 1)
            store.removePane(paneID)
            #expect(store.status(forPane: paneID) == nil)
            #expect(!store.isCompletionPending(forPane: paneID))
            send(store, paneID, .idle, sequence: 1)
            #expect(store.status(forPane: paneID) == .idle)
        }
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
