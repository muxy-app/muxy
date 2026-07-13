import AppKit
import Foundation

@MainActor
@Observable
final class TerminalProgressStore {
    static let shared = TerminalProgressStore()

    var appState: AppState?

    private(set) var progresses: [UUID: TerminalProgress] = [:]
    private(set) var completionPending: Set<UUID> = []
    private var paneToWorktree: [UUID: WorktreeKey] = [:]
    private var didBecomeActiveObserver: NSObjectProtocol?

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
        MainActor.assumeIsolated {
            if let didBecomeActiveObserver {
                NotificationCenter.default.removeObserver(didBecomeActiveObserver)
            }
        }
    }

    private func clearActivePaneCompletion() {
        guard let appState, let paneID = NotificationNavigator.activePaneID(appState: appState) else { return }
        clearCompletion(for: paneID)
        AgentStatusStore.shared.clearCompletion(for: paneID)
    }

    func setProgress(_ progress: TerminalProgress?, for paneID: UUID, worktreeKey: WorktreeKey?) {
        let existing = progresses[paneID]

        if let worktreeKey {
            paneToWorktree[paneID] = worktreeKey
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
        paneToWorktree.removeValue(forKey: paneID)
    }

    func progress(for paneID: UUID) -> TerminalProgress? {
        progresses[paneID]
    }

    func isCompletionPending(for paneID: UUID) -> Bool {
        completionPending.contains(paneID)
    }

    func hasCompletionPending(for projectID: UUID) -> Bool {
        completionPending.contains { paneToWorktree[$0]?.projectID == projectID }
    }

    func hasCompletionPending(forWorktree worktreeID: UUID) -> Bool {
        completionPending.contains { paneToWorktree[$0]?.worktreeID == worktreeID }
    }

    func hasActiveProgress(for projectID: UUID) -> Bool {
        progresses.keys.contains { paneToWorktree[$0]?.projectID == projectID }
    }

    func hasActiveProgress(forWorktree worktreeID: UUID) -> Bool {
        progresses.keys.contains { paneToWorktree[$0]?.worktreeID == worktreeID }
    }
}
