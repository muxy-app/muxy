import Darwin
import Foundation

enum AgentStatus: String, Equatable, Codable {
    case working
    case waiting
    case idle

    var priority: Int {
        switch self {
        case .working: 2
        case .waiting: 1
        case .idle: 0
        }
    }
}

enum AgentLifecyclePhase: String, Equatable {
    case working
    case waiting
    case finished

    var status: AgentStatus {
        switch self {
        case .working: .working
        case .waiting: .waiting
        case .finished: .idle
        }
    }
}

@MainActor
protocol AgentGraceScheduler {
    func schedule(after delay: TimeInterval, _ work: @escaping @MainActor () -> Void) -> AgentGraceCancellable
}

@MainActor
protocol AgentGraceCancellable {
    func cancel()
}

@MainActor
final class DispatchAgentGraceScheduler: AgentGraceScheduler {
    func schedule(after delay: TimeInterval, _ work: @escaping @MainActor () -> Void) -> AgentGraceCancellable {
        let item = DispatchWorkItem { work() }
        DispatchQueue.main.asyncAfter(deadline: .now() + delay, execute: item)
        return DispatchAgentGraceCancellable(item: item)
    }
}

@MainActor
final class DispatchAgentGraceCancellable: AgentGraceCancellable {
    private let item: DispatchWorkItem

    init(item: DispatchWorkItem) {
        self.item = item
    }

    func cancel() {
        item.cancel()
    }
}

@MainActor
@Observable
final class AgentStatusStore {
    static let shared = AgentStatusStore()

    struct Entry: Equatable {
        let worktreeID: UUID
        let projectID: UUID
        let paneID: UUID
        let providerID: String
        let status: AgentStatus
        let updatedAt: Date
        var sequence: UInt64 = 0
    }

    static let detectionLossGrace: TimeInterval = 4
    static let waitingDetectionLossGrace: TimeInterval = 30

    private(set) var entries: [UUID: Entry] = [:]
    private(set) var completionPending: Set<UUID> = []
    private var panes: [UUID: Entry] = [:]
    private var appliedSequence: [UUID: UInt64] = [:]
    private var pendingGrace: [UUID: AgentGraceCancellable] = [:]
    private var detectionLost: Set<UUID> = []
    private var detectedAgentProcessIDs: [UUID: Int32] = [:]
    private var sequenceCounter: UInt64 = 0

    private let scheduler: AgentGraceScheduler
    private let graceDelay: TimeInterval
    private let waitingGraceDelay: TimeInterval
    private let foregroundProcessID: @MainActor (UUID) -> Int32?
    private let processIsAlive: @MainActor (Int32) -> Bool
    private let agentProcessLivenessOverride: (@MainActor (UUID) -> Bool)?

    init(
        scheduler: AgentGraceScheduler = DispatchAgentGraceScheduler(),
        graceDelay: TimeInterval = AgentStatusStore.detectionLossGrace,
        waitingGraceDelay: TimeInterval = AgentStatusStore.waitingDetectionLossGrace,
        foregroundProcessID: @escaping @MainActor (UUID) -> Int32? = { paneID in
            TerminalViewRegistry.shared.existingView(for: paneID)?.foregroundProcessID
        },
        processIsAlive: @escaping @MainActor (Int32) -> Bool = AgentStatusStore.processIsAlive,
        isAgentProcessAlive: (@MainActor (UUID) -> Bool)? = nil
    ) {
        self.scheduler = scheduler
        self.graceDelay = graceDelay
        self.waitingGraceDelay = waitingGraceDelay
        self.foregroundProcessID = foregroundProcessID
        self.processIsAlive = processIsAlive
        agentProcessLivenessOverride = isAgentProcessAlive
    }

    func nextSequence() -> UInt64 {
        sequenceCounter += 1
        return sequenceCounter
    }

    func update(paneID: UUID, providerID: String, status: AgentStatus, sequence: UInt64, appState: AppState) {
        if let applied = appliedSequence[paneID], sequence <= applied {
            return
        }
        appliedSequence[paneID] = sequence

        if let existing = panes[paneID], existing.status == status, existing.providerID == providerID {
            resetGraceIfNeeded(for: paneID, status: status)
            return
        }

        guard let worktreeStore = NotificationStore.shared.worktreeStore,
              let context = NotificationNavigator.resolveContext(
                  for: paneID,
                  appState: appState,
                  worktreeStore: worktreeStore
              )
        else { return }

        cancelGrace(for: paneID)
        applyEntry(Entry(
            worktreeID: context.worktreeID,
            projectID: context.projectID,
            paneID: paneID,
            providerID: providerID,
            status: status,
            updatedAt: Date(),
            sequence: sequence
        ))
        scheduleGraceIfNeeded(for: paneID, status: status)
    }

    func removePane(_ paneID: UUID) {
        cancelGrace(for: paneID)
        detectionLost.remove(paneID)
        detectedAgentProcessIDs.removeValue(forKey: paneID)
        appliedSequence.removeValue(forKey: paneID)
        completionPending.remove(paneID)
        guard let removed = panes.removeValue(forKey: paneID) else { return }
        recompute(worktreeID: removed.worktreeID)
    }

    func noteDetectionActive(paneID: UUID, processID: Int32?) {
        detectionLost.remove(paneID)
        cancelGrace(for: paneID)
        if let processID {
            detectedAgentProcessIDs[paneID] = processID
            return
        }
        detectedAgentProcessIDs.removeValue(forKey: paneID)
    }

    func noteDetectionActive(paneID: UUID) {
        noteDetectionActive(paneID: paneID, processID: foregroundProcessID(paneID))
    }

    func noteDetectionLost(paneID: UUID) {
        detectionLost.insert(paneID)
        guard let existing = panes[paneID], existing.status != .idle else { return }
        scheduleGraceIfNeeded(for: paneID, status: existing.status)
    }

    func clearCompletion(for paneID: UUID) {
        completionPending.remove(paneID)
    }

    func status(forPane paneID: UUID?) -> AgentStatus? {
        guard let paneID else { return nil }
        return panes[paneID]?.status
    }

    func status(forWorktree worktreeID: UUID) -> AgentStatus? {
        entries[worktreeID]?.status
    }

    func status(forProject projectID: UUID) -> AgentStatus? {
        Self.winningEntry(among: panes.values.filter { $0.projectID == projectID })?.status
    }

    func isCompletionPending(forPane paneID: UUID) -> Bool {
        completionPending.contains(paneID)
    }

    func hasCompletionPending(forWorktree worktreeID: UUID) -> Bool {
        completionPending.contains { panes[$0]?.worktreeID == worktreeID }
    }

    func hasCompletionPending(forProject projectID: UUID) -> Bool {
        completionPending.contains { panes[$0]?.projectID == projectID }
    }

    nonisolated static func marksCompletion(from previous: AgentStatus?, to current: AgentStatus) -> Bool {
        guard current == .idle else { return false }
        return previous == .working || previous == .waiting
    }

    nonisolated static func winningEntry(among candidates: [Entry]) -> Entry? {
        candidates.max { lhs, rhs in
            lhs.status.priority != rhs.status.priority
                ? lhs.status.priority < rhs.status.priority
                : lhs.updatedAt < rhs.updatedAt
        }
    }

    private func resolveGrace(for paneID: UUID) {
        pendingGrace.removeValue(forKey: paneID)
        guard detectionLost.contains(paneID) else { return }
        guard let existing = panes[paneID], existing.status != .idle else { return }
        if existing.status == .waiting, isDetectedAgentProcessAlive(paneID: paneID) {
            scheduleGraceIfNeeded(for: paneID, status: existing.status)
            return
        }
        let sequence = nextSequence()
        appliedSequence[paneID] = sequence
        applyEntry(Entry(
            worktreeID: existing.worktreeID,
            projectID: existing.projectID,
            paneID: paneID,
            providerID: existing.providerID,
            status: .idle,
            updatedAt: Date(),
            sequence: sequence
        ))
    }

    private func isDetectedAgentProcessAlive(paneID: UUID) -> Bool {
        if let agentProcessLivenessOverride {
            return agentProcessLivenessOverride(paneID)
        }
        guard let detectedProcessID = detectedAgentProcessIDs[paneID] else { return false }
        return processIsAlive(detectedProcessID)
    }

    private func resetGraceIfNeeded(for paneID: UUID, status: AgentStatus) {
        cancelGrace(for: paneID)
        guard detectedAgentProcessIDs[paneID] != nil else { return }
        scheduleGraceIfNeeded(for: paneID, status: status)
    }

    private func scheduleGraceIfNeeded(for paneID: UUID, status: AgentStatus) {
        guard detectionLost.contains(paneID), status != .idle else { return }
        guard pendingGrace[paneID] == nil else { return }
        let delay = status == .waiting ? waitingGraceDelay : graceDelay
        pendingGrace[paneID] = scheduler.schedule(after: delay) { [weak self] in
            self?.resolveGrace(for: paneID)
        }
    }

    private func cancelGrace(for paneID: UUID) {
        pendingGrace.removeValue(forKey: paneID)?.cancel()
    }

    nonisolated private static func processIsAlive(_ processID: Int32) -> Bool {
        guard processID > 0 else { return false }
        if Darwin.kill(processID, 0) == 0 {
            return true
        }
        return errno == EPERM
    }

    private func applyEntry(_ entry: Entry) {
        let existingStatus = panes[entry.paneID]?.status
        panes[entry.paneID] = entry
        updateCompletion(paneID: entry.paneID, from: existingStatus, to: entry.status)
        recompute(worktreeID: entry.worktreeID)
    }

    private func updateCompletion(paneID: UUID, from previous: AgentStatus?, to current: AgentStatus) {
        if Self.marksCompletion(from: previous, to: current) {
            completionPending.insert(paneID)
            return
        }
        guard current != .idle else { return }
        completionPending.remove(paneID)
    }

    private func recompute(worktreeID: UUID) {
        let candidates = panes.values.filter { $0.worktreeID == worktreeID }

        guard let aggregate = Self.winningEntry(among: candidates) else {
            guard let previous = entries.removeValue(forKey: worktreeID), previous.status != .idle else { return }
            broadcast(
                worktreeID: previous.worktreeID,
                projectID: previous.projectID,
                paneID: previous.paneID,
                providerID: previous.providerID,
                status: .idle
            )
            return
        }

        if let existing = entries[worktreeID],
           existing.status == aggregate.status,
           existing.paneID == aggregate.paneID,
           existing.providerID == aggregate.providerID
        {
            return
        }

        entries[worktreeID] = aggregate
        broadcast(
            worktreeID: aggregate.worktreeID,
            projectID: aggregate.projectID,
            paneID: aggregate.paneID,
            providerID: aggregate.providerID,
            status: aggregate.status
        )
    }

    private func broadcast(
        worktreeID: UUID,
        projectID: UUID,
        paneID: UUID,
        providerID: String,
        status: AgentStatus
    ) {
        NotificationSocketServer.shared.broadcast(event: ExtensionEvent(
            name: ExtensionEventName.agentStatus,
            payload: Self.eventPayload(
                worktreeID: worktreeID,
                projectID: projectID,
                paneID: paneID,
                providerID: providerID,
                status: status
            )
        ))
    }

    nonisolated static func eventPayload(
        worktreeID: UUID,
        projectID: UUID,
        paneID: UUID,
        providerID: String,
        status: AgentStatus
    ) -> [String: String] {
        [
            "worktreeID": worktreeID.uuidString,
            "projectID": projectID.uuidString,
            "paneID": paneID.uuidString,
            "providerID": providerID,
            "status": status.rawValue,
        ]
    }
}
