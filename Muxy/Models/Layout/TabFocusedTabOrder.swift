import Foundation

@MainActor
enum TabFocusedTabOrder {
    struct Entry {
        let projectID: UUID
        let worktreeID: UUID?
        let areaID: UUID
        let tabID: UUID
    }

    static func orderedProjects(
        projectStore: ProjectStore,
        projectGroupStore: ProjectGroupStore
    ) -> [Project] {
        let stored = projectGroupStore.displayProjects(localProjects: projectStore.storedProjects)
        guard HomeProjectPreferences.isVisible else { return stored }
        if projectGroupStore.isRemoteWorkspaceActive {
            guard let home = projectGroupStore.activeRemoteHomeProject else { return stored }
            return [home] + stored
        }
        return [Project.home] + stored
    }

    static func entries(
        appState: AppState,
        projectStore: ProjectStore,
        projectGroupStore: ProjectGroupStore,
        worktreeStore: WorktreeStore,
        expansionStore: TabFocusedSidebarState = .shared
    ) -> [Entry] {
        orderedProjects(projectStore: projectStore, projectGroupStore: projectGroupStore)
            .filter { project in
                if expansionStore.focusMode, let activeID = appState.activeProjectID {
                    return project.id == activeID
                }
                return true
            }
            .flatMap { project -> [Entry] in
                worktreeRows(for: project, worktreeStore: worktreeStore).flatMap { worktree -> [Entry] in
                    guard expansionStore.isExpandedPersisted(worktree.isPrimary ? project.id : worktree.id) else {
                        return []
                    }
                    let key = WorktreeKey(projectID: project.id, worktreeID: worktree.id)
                    return appState.areas(for: key).flatMap { area in
                        area.tabs.map { Entry(projectID: project.id, worktreeID: worktree.id, areaID: area.id, tabID: $0.id) }
                    }
                }
            }
    }

    private static func worktreeRows(for project: Project, worktreeStore: WorktreeStore) -> [Worktree] {
        let worktrees = worktreeStore.list(for: project.id)
        guard project.worktreesEnabled, !project.isHome else {
            return worktrees.filter(\.isPrimary)
        }
        return worktrees
    }
}
