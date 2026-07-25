import Foundation

@MainActor
enum TabFocusedSidebarTarget {
    static func activate(
        project: Project,
        worktree: Worktree?,
        appState: AppState,
        worktreeStore: WorktreeStore
    ) {
        if appState.activeProjectID != project.id {
            worktreeStore.ensurePrimary(for: project)
            if let preferred = worktreeStore.preferred(
                for: project.id,
                matching: appState.activeWorktreeID[project.id]
            ) {
                appState.selectProject(project, worktree: worktree ?? preferred)
            }
        }
        if let worktree, appState.activeWorktreeID[project.id] != worktree.id {
            appState.selectWorktree(projectID: project.id, worktree: worktree)
        }
    }
}
