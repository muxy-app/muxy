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
    }

    private(set) var entries: [UUID: Entry] = [:]
    private(set) var completionPending: Set<UUID> = []
    private var panes: [UUID: Entry] = [:]

    private init() {}

    func update(paneID: UUID, providerID: String, status: AgentStatus, appState: AppState) {
        if let existing = panes[paneID], existing.status == status, existing.providerID == providerID {
            return
        }

        guard let worktreeStore = NotificationStore.shared.worktreeStore,
              let context = NotificationNavigator.resolveContext(
                  for: paneID,
                  appState: appState,
                  worktreeStore: worktreeStore
              )
        else { return }

        let existingStatus = panes[paneID]?.status
        panes[paneID] = Entry(
            worktreeID: context.worktreeID,
            projectID: context.projectID,
            paneID: paneID,
            providerID: providerID,
            status: status,
            updatedAt: Date()
        )
        updateCompletion(paneID: paneID, from: existingStatus, to: status)
        recompute(worktreeID: context.worktreeID)
    }

    func removePane(_ paneID: UUID) {
        completionPending.remove(paneID)
        guard let removed = panes.removeValue(forKey: paneID) else { return }
        recompute(worktreeID: removed.worktreeID)
    }

    func markIdleIfActive(paneID: UUID) {
        guard let existing = panes[paneID], existing.status != .idle else { return }
        panes[paneID] = Entry(
            worktreeID: existing.worktreeID,
            projectID: existing.projectID,
            paneID: paneID,
            providerID: existing.providerID,
            status: .idle,
            updatedAt: Date()
        )
        completionPending.insert(paneID)
        recompute(worktreeID: existing.worktreeID)
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
