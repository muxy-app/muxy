import AppKit
import Foundation

@MainActor
@Observable
final class TerminalProgressStore {
    static let shared = TerminalProgressStore()

    var appState: AppState?

    private(set) var progresses: [UUID: TerminalProgress] = [:]
    private(set) var completionPending: Set<UUID> = []
    private var paneToProject: [UUID: UUID] = [:]
    nonisolated(unsafe) private var didBecomeActiveObserver: NSObjectProtocol?

    init() {
        didBecomeActiveObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.didBecomeActiveNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in
                self?.clearActivePaneCompletion()
            }
        }
    }

    deinit {
        if let didBecomeActiveObserver {
            NotificationCenter.default.removeObserver(didBecomeActiveObserver)
        }
    }

    private func clearActivePaneCompletion() {
        guard let appState, let paneID = NotificationNavigator.activePaneID(appState: appState) else { return }
        clearCompletion(for: paneID)
    }

    func setProgress(_ progress: TerminalProgress?, for paneID: UUID, projectID: UUID?) {
        let existing = progresses[paneID]

        if let projectID {
            paneToProject[paneID] = projectID
        }

        if let progress {
            progresses[paneID] = progress
            return
        }

        progresses.removeValue(forKey: paneID)
        guard existing != nil else { return }
        completionPending.insert(paneID)
    }

    func clearCompletion(for paneID: UUID) {
        completionPending.remove(paneID)
    }

    func resetPane(_ paneID: UUID) {
        progresses.removeValue(forKey: paneID)
        completionPending.remove(paneID)
        paneToProject.removeValue(forKey: paneID)
    }

    func progress(for paneID: UUID) -> TerminalProgress? {
        progresses[paneID]
    }

    func isCompletionPending(for paneID: UUID) -> Bool {
        completionPending.contains(paneID)
    }

    func hasCompletionPending(for projectID: UUID) -> Bool {
        completionPending.contains { paneToProject[$0] == projectID }
    }
}
