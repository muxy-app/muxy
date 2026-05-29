import Foundation

struct WorktreeRemovalConfirmation: Identifiable, Equatable {
    let worktree: Worktree
    let title: String
    let message: String

    var id: UUID {
        worktree.id
    }

    init(worktree: Worktree, hasUncommittedChanges: Bool) {
        self.worktree = worktree
        title = "Remove worktree \"\(worktree.name)\"?"
        if hasUncommittedChanges {
            message = "This worktree has uncommitted changes. Removing it will permanently discard them."
            return
        }
        message = "This will remove the worktree from Muxy and delete its files on disk."
    }
}
