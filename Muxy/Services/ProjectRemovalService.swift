import Foundation

@MainActor
enum ProjectRemovalService {
    static func remove(
        _ project: Project,
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore,
        projectGroupStore: ProjectGroupStore
    ) async throws {
        guard !project.isRemote else {
            if let workspaceID = project.remoteWorkspaceID {
                projectGroupStore.removeRemoteProject(id: project.id, fromGroup: workspaceID)
            } else {
                projectStore.remove(id: project.id)
                projectGroupStore.removeProjectFromAllGroups(projectID: project.id)
            }
            appState.removeProject(project.id)
            worktreeStore.removeProject(project.id)
            return
        }

        let knownWorktrees = worktreeStore.list(for: project.id)
        try await WorktreeStore.cleanupOnDisk(for: project, knownWorktrees: knownWorktrees)
        appState.removeProject(project.id)
        projectStore.remove(id: project.id)
        worktreeStore.removeProject(project.id)
    }
}
