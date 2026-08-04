import Foundation

typealias WorktreeOfflineEmitting = @MainActor (WorktreeKey, String, Bool) -> Void

@MainActor
final class TerminalOfflineStore {
    static let shared = TerminalOfflineStore()

    private struct PaneEntry: Equatable {
        let worktreeKey: WorktreeKey
        let worktreePath: String
        let offline: Bool
    }

    private var panes: [UUID: PaneEntry] = [:]
    private var worktreeStates: [WorktreeKey: Bool] = [:]
    private let emitWorktreeOffline: WorktreeOfflineEmitting

    init(emitWorktreeOffline: @escaping WorktreeOfflineEmitting = ExtensionEventEmitter.emitWorktreeOffline) {
        self.emitWorktreeOffline = emitWorktreeOffline
    }

    func addPane(_ paneID: UUID, worktreeKey: WorktreeKey, worktreePath: String) {
        guard panes[paneID] == nil else { return }
        panes[paneID] = PaneEntry(worktreeKey: worktreeKey, worktreePath: worktreePath, offline: false)
        recompute(worktreeKey: worktreeKey, worktreePath: worktreePath)
    }

    func update(paneID: UUID, appState: AppState) {
        guard let located = appState.locatePane(paneID: paneID) else { return }
        syncPanes(in: located.worktreeKey, appState: appState)
        recompute(worktreeKey: located.worktreeKey, worktreePath: located.pane.projectPath)
    }

    func removePane(_ paneID: UUID) {
        removePanes([paneID])
    }

    func removePanes<PaneIDs: Sequence>(_ paneIDs: PaneIDs) where PaneIDs.Element == UUID {
        var affectedWorktrees: [WorktreeKey: String] = [:]
        for paneID in paneIDs {
            guard let removed = panes.removeValue(forKey: paneID) else { continue }
            affectedWorktrees[removed.worktreeKey] = removed.worktreePath
        }
        for (worktreeKey, worktreePath) in affectedWorktrees {
            recompute(worktreeKey: worktreeKey, worktreePath: worktreePath)
        }
    }

    func state(for worktreeKey: WorktreeKey) -> Bool? {
        worktreeStates[worktreeKey]
    }

    private func syncPanes(in worktreeKey: WorktreeKey, appState: AppState) {
        let currentPanes = appState.workspaceRoots[worktreeKey]?.allAreas()
            .flatMap(\.tabs)
            .compactMap(\.content.pane) ?? []
        let currentPaneIDs = Set(currentPanes.map(\.id))
        panes = panes.filter { entry in
            entry.value.worktreeKey != worktreeKey || currentPaneIDs.contains(entry.key)
        }
        for pane in currentPanes {
            panes[pane.id] = PaneEntry(
                worktreeKey: worktreeKey,
                worktreePath: pane.projectPath,
                offline: pane.isOffline
            )
        }
    }

    private func recompute(worktreeKey: WorktreeKey, worktreePath: String) {
        let entries = panes.values.filter { $0.worktreeKey == worktreeKey }
        guard !entries.isEmpty else {
            guard worktreeStates.removeValue(forKey: worktreeKey) == true else { return }
            emitWorktreeOffline(worktreeKey, worktreePath, false)
            return
        }
        let aggregate = entries.allSatisfy(\.offline)
        guard worktreeStates[worktreeKey] != aggregate else { return }
        let previous = worktreeStates.updateValue(aggregate, forKey: worktreeKey)
        guard previous != nil || aggregate else { return }
        emitWorktreeOffline(worktreeKey, worktreePath, aggregate)
    }
}
