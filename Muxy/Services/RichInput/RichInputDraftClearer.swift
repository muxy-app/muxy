import Foundation

@MainActor
enum RichInputDraftClearer {
    static func clear(
        target: RichInputPresentationTarget?,
        states: [WorktreeKey: RichInputState],
        store: RichInputDraftStore
    ) {
        guard let worktreeKey = target?.worktreeKey else { return }
        states[worktreeKey]?.clear()
        store.scheduleSave(.empty, for: worktreeKey)
    }
}
