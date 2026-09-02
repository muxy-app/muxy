import SwiftUI

enum ProjectActionsContextMenuPolicy {
    struct Context {
        let isGitRepo: Bool
        let isCheckingGitRepo: Bool
        let worktreeCount: Int
        let supportsSwitchWorktree: Bool
        let hasLocalWorkspaces: Bool
        let hasRemoteWorkspaces: Bool
    }

    enum Feature: Hashable {
        case pin
        case worktreeActions
        case loadingWorktrees
        case switchWorktree
        case workspaceMembership
    }

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

    static func features(
        for project: Project,
        context: Context
    ) -> Set<Feature> {
        var features: Set<Feature> = []
        if showsPin(isHome: project.isHome) {
            features.insert(.pin)
        }
        if showsWorktreeActions(isGitRepo: context.isGitRepo) {
            features.insert(.worktreeActions)
        }
        if showsLoadingWorktrees(
            isGitRepo: context.isGitRepo,
            isCheckingGitRepo: context.isCheckingGitRepo
        ) {
            features.insert(.loadingWorktrees)
        }
        if features.contains(.worktreeActions), showsSwitchWorktree(
            worktreesEnabled: project.worktreesEnabled,
            worktreeCount: context.worktreeCount,
            supportsSwitchWorktree: context.supportsSwitchWorktree
        ) {
            features.insert(.switchWorktree)
        }
        if showsWorkspaceMembership(for: project, context: context) {
            features.insert(.workspaceMembership)
        }
        return features
    }

    static func showsWorkspaceMembership(for project: Project, context: Context) -> Bool {
        if project.remoteWorkspaceID != nil {
            return context.hasRemoteWorkspaces
        }
        return project.supportsLocalWorkspaceMembership && context.hasLocalWorkspaces
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

    private var features: Set<ProjectActionsContextMenuPolicy.Feature> {
        ProjectActionsContextMenuPolicy.features(
            for: project,
            context: ProjectActionsContextMenuPolicy.Context(
                isGitRepo: isGitRepo,
                isCheckingGitRepo: isCheckingGitRepo,
                worktreeCount: worktreeCount,
                supportsSwitchWorktree: onSwitchWorktree != nil,
                hasLocalWorkspaces: projectGroupStore.groups.contains { $0.type == .local },
                hasRemoteWorkspaces: !projectGroupStore.remoteWorkspaceMoveTargets(for: project).isEmpty
            )
        )
    }

    var body: some View {
        if features.contains(.pin) {
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
        if features.contains(.workspaceMembership) {
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
        if features.contains(.worktreeActions) {
            Divider()
            Toggle(L10n.string("Worktrees"), isOn: worktreesEnabledBinding)
            if project.worktreesEnabled {
                Button(L10n.string("Refresh Worktrees"), action: onRefreshWorktrees)
                Button(L10n.string("New Worktree…"), action: onCreateWorktree)
                if features.contains(.switchWorktree), let onSwitchWorktree {
                    Button(L10n.string("Switch Worktree…"), action: onSwitchWorktree)
                }
            }
        } else if features.contains(.loadingWorktrees) {
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
