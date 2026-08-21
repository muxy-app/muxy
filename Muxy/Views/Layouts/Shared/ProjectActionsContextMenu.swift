import SwiftUI

enum ProjectActionsContextMenuPolicy {
    static func showsPin(isHome: Bool) -> Bool {
        !isHome
    }

    static func showsWorktreeActions(isGitRepo: Bool) -> Bool {
        isGitRepo
    }

    static func showsLoadingWorktrees(isGitRepo: Bool, isCheckingGitRepo: Bool) -> Bool {
        !isGitRepo && isCheckingGitRepo
    }

    static func showsSwitchWorktree(
        worktreesEnabled: Bool,
        worktreeCount: Int,
        supportsSwitchWorktree: Bool
    ) -> Bool {
        worktreesEnabled && worktreeCount > 1 && supportsSwitchWorktree
    }
}

struct ProjectActionsContextMenu: View {
    let project: Project
    let editor: ProjectEditingService
    let isGitRepo: Bool
    let isCheckingGitRepo: Bool
    let worktreeCount: Int
    let onPickLogo: () -> Void
    let onPickIcon: () -> Void
    let onPickIconColor: () -> Void
    let onRename: () -> Void
    let onSetWorktreesEnabled: (Bool) -> Void
    let onRefreshWorktrees: () -> Void
    let onCreateWorktree: () -> Void
    let onSwitchWorktree: (() -> Void)?
    let onRemove: () -> Void

    @Environment(ProjectGroupStore.self) private var projectGroupStore

    private var canMoveToWorkspace: Bool {
        !project.isRemote && projectGroupStore.groups.contains { $0.type == .local }
    }

    var body: some View {
        if ProjectActionsContextMenuPolicy.showsPin(isHome: project.isHome) {
            Button(project.isPinned ? L10n.string("Unpin") : L10n.string("Pin")) {
                editor.setPinned(project, to: !project.isPinned)
            }
            Divider()
        }
        Button(L10n.string("Set Logo..."), action: onPickLogo)
        if project.logo != nil {
            Button(L10n.string("Remove Logo")) { editor.setLogo(project, to: nil) }
        }
        Button(L10n.string("Set Icon..."), action: onPickIcon)
        if project.icon != nil {
            Button(L10n.string("Remove Icon")) { editor.setIcon(project, to: nil) }
        }
        Button(L10n.string("Set Icon Color..."), action: onPickIconColor)
        if project.iconColor != nil {
            Button(L10n.string("Reset Icon Color")) { editor.setIconColor(project, to: nil) }
        }
        Divider()
        Button(L10n.string("Rename Project"), action: onRename)
        worktreeActions
        if canMoveToWorkspace {
            Divider()
            ProjectGroupMembershipMenu(project: project)
        }
        ProjectContextMenuFooter(
            path: project.path,
            workspaceContext: projectGroupStore.workspaceContext(for: project)
        ) {
            Button(L10n.string("Remove Project"), role: .destructive, action: onRemove)
        }
    }

    @ViewBuilder
    private var worktreeActions: some View {
        if ProjectActionsContextMenuPolicy.showsWorktreeActions(isGitRepo: isGitRepo) {
            Divider()
            Toggle(L10n.string("Worktrees"), isOn: worktreesEnabledBinding)
            if project.worktreesEnabled {
                Button(L10n.string("Refresh Worktrees"), action: onRefreshWorktrees)
                Button(L10n.string("New Worktree…"), action: onCreateWorktree)
                if ProjectActionsContextMenuPolicy.showsSwitchWorktree(
                    worktreesEnabled: project.worktreesEnabled,
                    worktreeCount: worktreeCount,
                    supportsSwitchWorktree: onSwitchWorktree != nil
                ), let onSwitchWorktree {
                    Button(L10n.string("Switch Worktree…"), action: onSwitchWorktree)
                }
            }
        } else if ProjectActionsContextMenuPolicy.showsLoadingWorktrees(
            isGitRepo: isGitRepo,
            isCheckingGitRepo: isCheckingGitRepo
        ) {
            Divider()
            Button(L10n.string("Loading Worktrees…")) {}
                .disabled(true)
        }
    }

    private var worktreesEnabledBinding: Binding<Bool> {
        Binding(
            get: { project.worktreesEnabled },
            set: { onSetWorktreesEnabled($0) }
        )
    }
}
