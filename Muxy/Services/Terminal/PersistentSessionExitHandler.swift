import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "PersistentSession")

@MainActor
final class PersistentSessionExitHandler {
    static let shared = PersistentSessionExitHandler()

    enum Decision: Equatable {
        case closePane
        case reattach
        case reportFailure
    }

    static let attemptLimit = 3
    static let attemptResetInterval: TimeInterval = 60

    private struct Recovery {
        var attempt: Int
        var updatedAt: Date
    }

    private var recoveries: [UUID: Recovery] = [:]
    private var handlingPaneIDs: Set<UUID> = []

    private init() {}

    func handleExit(paneID: UUID, sessionID: UUID, closePane: @escaping () -> Void) {
        guard !handlingPaneIDs.contains(paneID) else { return }
        handlingPaneIDs.insert(paneID)
        let attempt = nextAttempt(paneID: paneID)

        Task { @MainActor [weak self] in
            let lookup = await PersistentSessionService.shared.lookup(sessionID: sessionID)
            guard let self else { return }
            defer { handlingPaneIDs.remove(paneID) }

            let decision = Self.decision(lookup: lookup, attempt: attempt, limit: Self.attemptLimit)
            guard TerminalViewRegistry.shared.hasPersistentSession(for: paneID, sessionID: sessionID) else { return }

            switch decision {
            case .closePane:
                recoveries.removeValue(forKey: paneID)
                closePane()
            case .reattach:
                logger.info("reattaching background session after attempt \(attempt)")
                try? await Task.sleep(for: Self.backoff(attempt: attempt))
                guard TerminalViewRegistry.shared.hasPersistentSession(for: paneID, sessionID: sessionID) else { return }
                recoverySurface(paneID: paneID)?.reattachPersistentSession()
            case .reportFailure:
                logger.error("giving up on reattaching a background session after \(attempt - 1) attempt(s)")
                recoveries.removeValue(forKey: paneID)
                recoverySurface(paneID: paneID)?.reportSessionRecoveryFailure()
            }
        }
    }

    func resetPane(_ paneID: UUID) {
        recoveries.removeValue(forKey: paneID)
        handlingPaneIDs.remove(paneID)
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

    nonisolated static func decision(lookup: PersistentSessionLookup, attempt: Int, limit: Int) -> Decision {
        guard lookup != .absent else { return .closePane }
        return attempt <= limit ? .reattach : .reportFailure
    }

    nonisolated static func backoff(attempt: Int) -> Duration {
        .milliseconds(300 * max(attempt, 1))
    }
}
