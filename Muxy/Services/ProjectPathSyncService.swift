import Foundation

@MainActor
enum ProjectPathSyncService {
    static func syncFromTerminalWorkingDirectory(
        projectID: UUID,
        worktreeID: UUID,
        path: String,
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore
    ) {
        guard appState.terminalCount(for: projectID) == 1 else { return }
        guard let project = projectStore.updatePath(id: projectID, to: path) else { return }
        worktreeStore.updatePrimary(projectID: projectID, path: project.path)
        appState.updateWorkspacePath(projectID: projectID, worktreeID: worktreeID, to: project.path)
    }
}
