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
            .filter { expansionStore.isExpandedPersisted($0.id) }
            .flatMap { project -> [Entry] in
                guard expansionStore.isGroupedByWorktree(project.id) else {
                    return appState.allAreas(for: project.id).flatMap { area in
                        area.tabs.map { Entry(projectID: project.id, worktreeID: nil, areaID: area.id, tabID: $0.id) }
                    }
                }
                return worktreeStore.list(for: project.id).flatMap { worktree -> [Entry] in
                    let key = WorktreeKey(projectID: project.id, worktreeID: worktree.id)
                    return appState.areas(for: key).flatMap { area in
                        area.tabs.map { Entry(projectID: project.id, worktreeID: worktree.id, areaID: area.id, tabID: $0.id) }
                    }
                }
            }
    }
}
