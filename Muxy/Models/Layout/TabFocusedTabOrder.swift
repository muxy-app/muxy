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
        expansionStore: TabFocusedSidebarState = .shared,
        groupWorktrees: Bool = WorktreeListPreferences.isGroupingEnabled
    ) -> [Entry] {
        orderedProjects(projectStore: projectStore, projectGroupStore: projectGroupStore)
            .filter { project in
                if expansionStore.focusMode, let activeID = appState.activeProjectID {
                    return project.id == activeID
                }
                return true
            }
            .flatMap { project -> [Entry] in
                let grouped = groupWorktrees && showsWorktreeRows(project)
                return worktreeRows(for: project, worktreeStore: worktreeStore).flatMap { worktree -> [Entry] in
                    guard isRowExpanded(
                        worktree: worktree,
                        projectID: project.id,
                        grouped: grouped,
                        isExpandedPersisted: { expansionStore.isExpandedPersisted($0) }
                    )
                    else {
                        return []
                    }
                    let key = WorktreeKey(projectID: project.id, worktreeID: worktree.id)
                    return appState.topLevelTabs(for: key).map { item in
                        Entry(
                            projectID: project.id,
                            worktreeID: worktree.id,
                            areaID: item.area.id,
                            tabID: item.tab.id
                        )
                    }
                }
            }
    }

    static func isRowExpanded(
        worktree: Worktree,
        projectID: UUID,
        grouped: Bool,
        isExpandedPersisted: (UUID) -> Bool
    ) -> Bool {
        guard grouped else {
            return isExpandedPersisted(worktree.isPrimary ? projectID : worktree.id)
        }
        return isExpandedPersisted(projectID) && isExpandedPersisted(worktree.id)
    }

    private static func showsWorktreeRows(_ project: Project) -> Bool {
        project.worktreesEnabled && !project.isHome
    }

    private static func worktreeRows(for project: Project, worktreeStore: WorktreeStore) -> [Worktree] {
        let worktrees = worktreeStore.list(for: project.id)
        guard showsWorktreeRows(project) else {
            return worktrees.filter(\.isPrimary)
        }
        return worktrees
    }
}
