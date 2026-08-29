import Foundation
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

    @Test("device-backed remote projects have local project feature parity")
    func deviceBackedRemoteProjectFeatureParity() {
        var localProject = Project(name: "Local", path: "/code/local")
        localProject.worktreesEnabled = true
        var remoteProject = Project(
            name: "Remote",
            path: "~/code/remote",
            remoteDeviceID: UUID()
        )
        remoteProject.worktreesEnabled = true

        let localFeatures = ProjectActionsContextMenuPolicy.features(
            for: localProject,
            context: parityContext
        )
        let remoteFeatures = ProjectActionsContextMenuPolicy.features(
            for: remoteProject,
            context: parityContext
        )

        #expect(localFeatures == remoteFeatures)
        #expect(localFeatures.contains(.workspaceMembership))
    }

    @Test("SSH workspace projects do not expose workspace membership without move targets")
    func sshWorkspaceProjectMembershipWithoutTargets() {
        let project = Project(
            name: "Remote",
            path: "~/code/remote",
            remoteWorkspaceID: UUID()
        )

        let features = ProjectActionsContextMenuPolicy.features(
            for: project,
            context: ProjectActionsContextMenuPolicy.Context(
                isGitRepo: true,
                isCheckingGitRepo: false,
                worktreeCount: 1,
                supportsSwitchWorktree: true,
                hasLocalWorkspaces: true,
                hasRemoteWorkspaces: false
            )
        )

        #expect(!features.contains(.workspaceMembership))
    }

    @Test("SSH workspace projects expose workspace membership with move targets")
    func sshWorkspaceProjectMembershipWithTargets() {
        let project = Project(
            name: "Remote",
            path: "~/code/remote",
            remoteWorkspaceID: UUID()
        )

        let features = ProjectActionsContextMenuPolicy.features(
            for: project,
            context: ProjectActionsContextMenuPolicy.Context(
                isGitRepo: true,
                isCheckingGitRepo: false,
                worktreeCount: 1,
                supportsSwitchWorktree: true,
                hasLocalWorkspaces: false,
                hasRemoteWorkspaces: true
            )
        )

        #expect(features.contains(.workspaceMembership))
    }

    @Test("non-Git projects do not expose switch worktree")
    func nonGitProjectSwitchWorktree() {
        var project = Project(name: "Folder", path: "/code/folder")
        project.worktreesEnabled = true

        let features = ProjectActionsContextMenuPolicy.features(
            for: project,
            context: ProjectActionsContextMenuPolicy.Context(
                isGitRepo: false,
                isCheckingGitRepo: false,
                worktreeCount: 2,
                supportsSwitchWorktree: true,
                hasLocalWorkspaces: false,
                hasRemoteWorkspaces: false
            )
        )

        #expect(!features.contains(.worktreeActions))
        #expect(!features.contains(.switchWorktree))
    }

    private var parityContext: ProjectActionsContextMenuPolicy.Context {
        ProjectActionsContextMenuPolicy.Context(
            isGitRepo: true,
            isCheckingGitRepo: false,
            worktreeCount: 2,
            supportsSwitchWorktree: true,
            hasLocalWorkspaces: true,
            hasRemoteWorkspaces: false
        )
    }
}
