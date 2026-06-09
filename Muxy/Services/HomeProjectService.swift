import Foundation

@MainActor
enum HomeProjectService {
    private static var homeDirectory: String {
        FileManager.default.homeDirectoryForCurrentUser.path
    }

    @discardableResult
    static func openHomeTab(
        appState: AppState,
        worktreeStore: WorktreeStore
    ) -> Bool {
        guard HomeProjectPreferences.isVisible else {
            return openHomeDirectoryTabInActiveProject(appState: appState, worktreeStore: worktreeStore)
        }
        return openHomeProjectTab(appState: appState, worktreeStore: worktreeStore)
    }

    private static func openHomeProjectTab(
        appState: AppState,
        worktreeStore: WorktreeStore
    ) -> Bool {
        let home = Project.home
        worktreeStore.ensurePrimary(for: home)
        guard let worktree = worktreeStore.preferred(
            for: home.id,
            matching: appState.activeWorktreeID[home.id]
        )
        else { return false }
        let hadWorkspace = appState.workspaceRoot(for: home.id) != nil
        appState.selectProject(home, worktree: worktree)
        guard hadWorkspace else { return true }
        appState.createTab(projectID: home.id)
        return true
    }

    private static func openHomeDirectoryTabInActiveProject(
        appState: AppState,
        worktreeStore: WorktreeStore
    ) -> Bool {
        guard let projectID = appState.activeProjectID else { return false }
        if appState.workspaceRoot(for: projectID) == nil {
            guard let worktree = worktreeStore.preferred(
                for: projectID,
                matching: appState.activeWorktreeID[projectID]
            )
            else { return false }
            appState.selectWorktree(projectID: projectID, worktree: worktree)
        }
        appState.dispatch(.createTabInDirectory(projectID: projectID, areaID: nil, directory: homeDirectory))
        return true
    }
}
