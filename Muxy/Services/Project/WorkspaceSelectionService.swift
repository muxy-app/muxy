import Foundation

@MainActor
enum WorkspaceSelectionService {
    static func selectFirstProject(
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore,
        projectGroupStore: ProjectGroupStore
    ) {
        guard let project = firstProject(
            projectStore: projectStore,
            projectGroupStore: projectGroupStore
        )
        else { return }
        worktreeStore.ensurePrimary(for: project)
        guard let worktree = worktreeStore.preferred(
            for: project.id,
            matching: appState.activeWorktreeID[project.id]
        )
        else { return }
        appState.selectProject(project, worktree: worktree)
    }

    private static func firstProject(
        projectStore: ProjectStore,
        projectGroupStore: ProjectGroupStore
    ) -> Project? {
        if HomeProjectPreferences.isVisible, let home = homeProject(projectGroupStore: projectGroupStore) {
            return home
        }
        return projectGroupStore.displayProjects(localProjects: projectStore.storedProjects).first
    }

    private static func homeProject(projectGroupStore: ProjectGroupStore) -> Project? {
        guard !projectGroupStore.isRemoteWorkspaceActive else {
            return projectGroupStore.activeRemoteHomeProject
        }
        return Project.home
    }
}
