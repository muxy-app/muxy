import Foundation

@MainActor
enum ProjectRemovalService {
    enum RemovalError: LocalizedError {
        case persistenceFailed

        var errorDescription: String? {
            "Muxy could not save the project removal."
        }
    }

    static func remove(
        _ project: Project,
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore,
        projectGroupStore: ProjectGroupStore
    ) async throws {
        if project.isRemote {
            try await removeRemoteProject(
                project,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore,
                projectGroupStore: projectGroupStore
            )
            return
        }

        GitRepoStatusCache.shared.remove(
            path: project.path,
            context: projectGroupStore.workspaceContext(for: project)
        )

        try await removeLocalProjectData(
            project,
            projectStore: projectStore,
            worktreeStore: worktreeStore
        )
        appState.removeProject(project.id)
    }

    private static func removeRemoteProject(
        _ project: Project,
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore,
        projectGroupStore: ProjectGroupStore
    ) async throws {
        guard await worktreeStore.beginProjectRemoval(project.id) else {
            throw RemovalError.persistenceFailed
        }

        GitRepoStatusCache.shared.remove(
            path: project.path,
            context: projectGroupStore.workspaceContext(for: project)
        )

        if let workspaceID = project.remoteWorkspaceID {
            projectGroupStore.removeRemoteProject(id: project.id, fromGroup: workspaceID)
        } else {
            guard await projectStore.prepareRemovalWhenAvailable(id: project.id) else {
                worktreeStore.cancelProjectRemoval(project.id)
                throw RemovalError.persistenceFailed
            }
            guard projectStore.commitRemoval(id: project.id) else {
                worktreeStore.cancelProjectRemoval(project.id)
                throw RemovalError.persistenceFailed
            }
            projectGroupStore.removeProjectFromAllGroups(projectID: project.id)
        }

        appState.removeProject(project.id)
        worktreeStore.completeProjectRemoval(project.id)
    }

    static func removeLocalProjectData(
        _ project: Project,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore,
        cleanupOnDisk: (Project, [Worktree]) async throws -> Void = { project, worktrees in
            try await WorktreeStore.cleanupOnDisk(for: project, knownWorktrees: worktrees)
        }
    ) async throws {
        guard await projectStore.prepareRemovalWhenAvailable(id: project.id) else {
            throw RemovalError.persistenceFailed
        }
        guard await worktreeStore.beginProjectRemoval(project.id) else {
            projectStore.cancelRemoval(id: project.id)
            throw RemovalError.persistenceFailed
        }
        do {
            try await cleanupOnDisk(project, worktreeStore.list(for: project.id))
        } catch {
            worktreeStore.cancelProjectRemoval(project.id)
            projectStore.cancelRemoval(id: project.id)
            throw error
        }
        guard projectStore.commitRemoval(id: project.id) else {
            worktreeStore.cancelProjectRemoval(project.id)
            throw RemovalError.persistenceFailed
        }
        worktreeStore.completeProjectRemoval(project.id)
    }
}
