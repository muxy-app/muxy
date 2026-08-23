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

    private var recoveries: [UUID: Recovery] = [:]
    private var tasks: [UUID: Task<Void, Never>] = [:]

    private init() {}

    func handleExit(paneID: UUID, session: RemoteTmuxSession, closePane: @escaping () -> Void) {
        guard tasks[paneID] == nil else { return }
        let backing = TerminalSessionBacking.remoteTmux(session)
        let attempt = nextAttempt(paneID: paneID)
        tasks[paneID] = Task { @MainActor [weak self] in
            let lookup = await RemoteTmuxSessionService.lookup(session)
            guard let self else { return }
            defer { tasks.removeValue(forKey: paneID) }
            guard TerminalViewRegistry.shared.hasSessionBacking(for: paneID, backing: backing) else { return }
            switch Self.decision(lookup: lookup, attempt: attempt, limit: Self.attemptLimit) {
            case .closePane:
                recoveries.removeValue(forKey: paneID)
                closePane()
            case .reattach:
                remoteTmuxLogger.info("reattaching remote tmux session after attempt \(attempt)")
                try? await Task.sleep(for: Self.backoff(attempt: attempt))
                guard !Task.isCancelled,
                      TerminalViewRegistry.shared.hasSessionBacking(for: paneID, backing: backing)
                else { return }
                recoverySurface(paneID: paneID)?.reattachSession()
            case .reportFailure:
                remoteTmuxLogger.error("giving up on reattaching remote tmux session after \(attempt - 1) attempt(s)")
                recoveries.removeValue(forKey: paneID)
                recoverySurface(paneID: paneID)?.reportSessionRecoveryFailure()
            }
        }
    }

    func resetPane(_ paneID: UUID) {
        tasks.removeValue(forKey: paneID)?.cancel()
        recoveries.removeValue(forKey: paneID)
    }

    private func recoverySurface(paneID: UUID) -> (any TerminalSessionRecoverySurface)? {
        TerminalViewRegistry.shared.existingView(for: paneID) as? any TerminalSessionRecoverySurface
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
