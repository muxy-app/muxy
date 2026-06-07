import Testing
@testable import Muxy

@MainActor
struct WorktreeRemovalConfirmationTests {
    @Test
    func cleanWorktreeConfirmationWarnsAboutDiskRemoval() {
        let worktree = Worktree(name: "feature", path: "/tmp/muxy-feature", branch: "feature", isPrimary: false)

        let confirmation = WorktreeRemovalConfirmation(
            worktree: worktree,
            hasUncommittedChanges: false
        )

        #expect(confirmation.title == "Remove worktree \"feature\"?")
        #expect(confirmation.message == "This will remove the worktree from Muxy and delete its files on disk.")
    }

    @Test
    func dirtyWorktreeConfirmationWarnsAboutDiscardedChanges() {
        let worktree = Worktree(name: "feature", path: "/tmp/muxy-feature", branch: "feature", isPrimary: false)

        let confirmation = WorktreeRemovalConfirmation(
            worktree: worktree,
            hasUncommittedChanges: true
        )

        #expect(confirmation.title == "Remove worktree \"feature\"?")
        #expect(confirmation.message == "This worktree has uncommitted changes. Removing it will permanently discard them.")
    }
}
