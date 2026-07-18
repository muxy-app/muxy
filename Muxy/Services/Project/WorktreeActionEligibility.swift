import Foundation

@MainActor
enum WorktreeActionEligibility {
    static func canCreateWorktree(
        project: Project?,
        worktreeStore: WorktreeStore,
        projectGroupStore: ProjectGroupStore,
        allowUnknownGitStatus: Bool
    ) -> Bool {
        guard let project, !project.isHome, project.worktreesEnabled else { return false }
        if worktreeStore.list(for: project.id).count > 1 {
            return true
        }
        let context = projectGroupStore.workspaceContext(for: project)
        return GitRepoStatusCache.shared.cachedStatus(for: project.path, context: context) ?? allowUnknownGitStatus
    }

    static func canCreateWorktreeResolvingGitStatus(
        project: Project,
        worktreeStore: WorktreeStore,
        projectGroupStore: ProjectGroupStore
    ) async -> Bool {
        guard !project.isHome, project.worktreesEnabled else { return false }
        if worktreeStore.list(for: project.id).count > 1 {
            return true
        }
        let context = projectGroupStore.workspaceContext(for: project)
        if let cached = GitRepoStatusCache.shared.cachedStatus(for: project.path, context: context) {
            return cached
        }
        let isGitRepo = await GitWorktreeService.shared.isGitRepository(project.path, context: context)
        GitRepoStatusCache.shared.update(path: project.path, context: context, isGitRepo: isGitRepo)
        return isGitRepo
    }

    static func removableCurrentWorktree(
        project: Project?,
        appState: AppState,
        worktreeStore: WorktreeStore
    ) -> Worktree? {
        guard let project else { return nil }
        guard let worktree = worktreeStore.preferred(for: project.id, matching: appState.activeWorktreeID[project.id]) else { return nil }
        return worktree.canBeRemoved ? worktree : nil
    }
}
