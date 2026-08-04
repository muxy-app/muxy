import Foundation
import MuxySessionProtocol
import os

private let logger = Logger(subsystem: "app.muxy", category: "PersistentSession")

@MainActor
final class PersistentSessionRecovery {
    static let shared = PersistentSessionRecovery()

    private var hasRecovered = false

    private init() {}

    func recoverRestoredPanes(appState: AppState) {
        guard !hasRecovered, TerminalPersistentSessionPreferences.isEnabled else { return }
        hasRecovered = true
        Task { @MainActor [weak self] in
            let sessions = await PersistentSessionService.shared.liveSessions()
            guard self != nil, !sessions.isEmpty else { return }
            let recovered = Self.reattach(sessions: sessions, appState: appState)
            guard recovered > 0 else { return }
            logger.info("reattached \(recovered) background terminal session(s)")
        }
    }

    @discardableResult
    static func reattach(sessions: [SessionDescriptor], appState: AppState) -> Int {
        let liveSessionIDs = Set(sessions.compactMap { UUID(uuidString: $0.identifier.uuidString) })
        var recovered = 0
        for pane in appState.allTerminalPanes where liveSessionIDs.contains(pane.sessionID) {
            guard TerminalViewRegistry.shared.existingView(for: pane.id) == nil else { continue }
            guard TerminalSurfaceMaterializer.materialize(paneID: pane.id, appState: appState) != nil else { continue }
            recovered += 1
        }
        return recovered
    }
}
