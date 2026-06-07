import Foundation

@MainActor
enum HomeProjectService {
    @discardableResult
    static func openHomeTab(
        appState: AppState,
        worktreeStore: WorktreeStore
    ) -> Bool {
        let home = Project.makeHome()
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
}
