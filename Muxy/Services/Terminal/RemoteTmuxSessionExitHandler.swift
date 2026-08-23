import Foundation
import os

private let remoteTmuxLogger = Logger(subsystem: "app.muxy", category: "RemoteTmuxSession")

@MainActor
final class RemoteTmuxSessionExitHandler {
    static let shared = RemoteTmuxSessionExitHandler()
    static let attemptLimit = 3
    static let attemptResetInterval: TimeInterval = 60

    enum Decision: Equatable {
        case closePane
        case reattach
        case reportFailure
    }

    private struct Recovery {
        var attempt: Int
        var updatedAt: Date
    }

    private struct RecoveryTask {
        let id: UUID
        let task: Task<Void, Never>
    }

    private var recoveries: [UUID: Recovery] = [:]
    private var tasks: [UUID: RecoveryTask] = [:]
    private let lookup: @MainActor (RemoteTmuxSession) async -> RemoteTmuxLookup
    private let hasSessionBacking: @MainActor (UUID, TerminalSessionBacking) -> Bool
    private let recoverySurface: @MainActor (UUID) -> (any TerminalSessionRecoverySurface)?

    init(
        lookup: @escaping @MainActor (RemoteTmuxSession) async -> RemoteTmuxLookup = {
            await RemoteTmuxSessionService.lookup($0)
        },
        hasSessionBacking: @escaping @MainActor (UUID, TerminalSessionBacking) -> Bool = {
            TerminalViewRegistry.shared.hasSessionBacking(for: $0, backing: $1)
        },
        recoverySurface: @escaping @MainActor (UUID) -> (any TerminalSessionRecoverySurface)? = {
            TerminalViewRegistry.shared.existingView(for: $0) as? any TerminalSessionRecoverySurface
        }
    ) {
        self.lookup = lookup
        self.hasSessionBacking = hasSessionBacking
        self.recoverySurface = recoverySurface
    }

    @discardableResult
    func handleExit(paneID: UUID, session: RemoteTmuxSession, closePane: @escaping () -> Void) -> Task<Void, Never>? {
        guard tasks[paneID] == nil else { return nil }
        let backing = TerminalSessionBacking.remoteTmux(session)
        let attempt = nextAttempt(paneID: paneID)
        let taskID = UUID()
        let task = Task { @MainActor [weak self] in
            guard let self else { return }
            let lookupResult = await lookup(session)
            defer { clearTask(paneID: paneID, taskID: taskID) }
            guard isCurrentTask(paneID: paneID, taskID: taskID),
                  hasSessionBacking(paneID, backing)
            else { return }
            switch Self.decision(lookup: lookupResult, attempt: attempt, limit: Self.attemptLimit) {
            case .closePane:
                recoveries.removeValue(forKey: paneID)
                closePane()
            case .reattach:
                remoteTmuxLogger.info("reattaching remote tmux session after attempt \(attempt)")
                try? await Task.sleep(for: Self.backoff(attempt: attempt))
                guard isCurrentTask(paneID: paneID, taskID: taskID),
                      hasSessionBacking(paneID, backing)
                else { return }
                recoverySurface(paneID)?.reattachSession()
            case .reportFailure:
                remoteTmuxLogger.error("giving up on reattaching remote tmux session after \(attempt - 1) attempt(s)")
                recoveries.removeValue(forKey: paneID)
                recoverySurface(paneID)?.reportSessionRecoveryFailure()
            }
        }
        tasks[paneID] = RecoveryTask(id: taskID, task: task)
        return task
    }

    func resetPane(_ paneID: UUID) {
        tasks.removeValue(forKey: paneID)?.task.cancel()
        recoveries.removeValue(forKey: paneID)
    }

    func hasRecoveryTask(for paneID: UUID) -> Bool {
        tasks[paneID] != nil
    }

    private func isCurrentTask(paneID: UUID, taskID: UUID) -> Bool {
        !Task.isCancelled && tasks[paneID]?.id == taskID
    }

    private func clearTask(paneID: UUID, taskID: UUID) {
        guard tasks[paneID]?.id == taskID else { return }
        tasks.removeValue(forKey: paneID)
    }

    private func nextAttempt(paneID: UUID, now: Date = Date()) -> Int {
        let previous = recoveries[paneID]
        let attempt = Self.attempt(
            previous: previous?.attempt ?? 0,
            elapsed: previous.map { now.timeIntervalSince($0.updatedAt) },
            resetAfter: Self.attemptResetInterval
        )
        recoveries[paneID] = Recovery(attempt: attempt, updatedAt: now)
        return attempt
    }

    nonisolated static func attempt(previous: Int, elapsed: TimeInterval?, resetAfter: TimeInterval) -> Int {
        guard let elapsed, elapsed < resetAfter else { return 1 }
        return previous + 1
    }

    nonisolated static func decision(lookup: RemoteTmuxLookup, attempt: Int, limit: Int) -> Decision {
        guard lookup != .absent else { return .closePane }
        return attempt <= limit ? .reattach : .reportFailure
    }

    nonisolated static func backoff(attempt: Int) -> Duration {
        .milliseconds(300 * max(attempt, 1))
    }
}
