import Testing

@testable import Muxy

@Suite("ProjectActionsContextMenuPolicy")
struct ProjectActionsContextMenuPolicyTests {
    @Test("pin is available for persisted projects only")
    func pinAvailability() {
        #expect(ProjectActionsContextMenuPolicy.showsPin(isHome: false))
        #expect(!ProjectActionsContextMenuPolicy.showsPin(isHome: true))
    }

    @Test("worktree actions distinguish repositories from pending checks")
    func worktreeAvailability() {
        #expect(ProjectActionsContextMenuPolicy.showsWorktreeActions(isGitRepo: true))
        #expect(!ProjectActionsContextMenuPolicy.showsWorktreeActions(isGitRepo: false))
        #expect(ProjectActionsContextMenuPolicy.showsLoadingWorktrees(
            isGitRepo: false,
            isCheckingGitRepo: true
        ))
        #expect(!ProjectActionsContextMenuPolicy.showsLoadingWorktrees(
            isGitRepo: true,
            isCheckingGitRepo: true
        ))
    }

    @Test("switch worktree requires the normal row capability and multiple worktrees")
    func switchWorktreeAvailability() {
        #expect(ProjectActionsContextMenuPolicy.showsSwitchWorktree(
            worktreesEnabled: true,
            worktreeCount: 2,
            supportsSwitchWorktree: true
        ))
        #expect(!ProjectActionsContextMenuPolicy.showsSwitchWorktree(
            worktreesEnabled: true,
            worktreeCount: 2,
            supportsSwitchWorktree: false
        ))
        #expect(!ProjectActionsContextMenuPolicy.showsSwitchWorktree(
            worktreesEnabled: true,
            worktreeCount: 1,
            supportsSwitchWorktree: true
        ))
        #expect(!ProjectActionsContextMenuPolicy.showsSwitchWorktree(
            worktreesEnabled: false,
            worktreeCount: 2,
            supportsSwitchWorktree: true
        ))
    }
}
