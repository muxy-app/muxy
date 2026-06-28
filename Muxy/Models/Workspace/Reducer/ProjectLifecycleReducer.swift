import Foundation

@MainActor
enum ProjectLifecycleReducer {
    struct WorktreeReplacement {
        let id: UUID
        let path: String
    }

    static func selectProject(
        projectID: UUID,
        worktreeID: UUID,
        worktreePath: String,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) {
        state.activeProjectID = projectID
        state.activeWorktreeID[projectID] = worktreeID
        WorkspaceReducerShared.ensureWorkspaceExists(
            projectID: projectID,
            worktreeID: worktreeID,
            worktreePath: worktreePath,
            state: &state,
            effects: &effects
        )
    }

    static func removeProject(
        projectID: UUID,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) {
        let keysToRemove = state.workspaceRoots.keys.filter { $0.projectID == projectID }
        for key in keysToRemove {
            if let root = state.workspaceRoots[key] {
                let paneIDs = root.allAreas().flatMap { area in area.tabs.compactMap { $0.content.pane?.id } }
                effects.paneIDsToRemove.append(contentsOf: paneIDs)
            }
            state.workspaceRoots.removeValue(forKey: key)
            state.focusedAreaID.removeValue(forKey: key)
            state.focusHistory.removeValue(forKey: key)
        }
        state.activeWorktreeID.removeValue(forKey: projectID)
        if state.activeProjectID == projectID {
            state.activeProjectID = nil
        }
    }

    static func removeWorktree(
        projectID: UUID,
        worktreeID: UUID,
        replacement: WorktreeReplacement?,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) {
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        if let root = state.workspaceRoots[key] {
            let paneIDs = root.allAreas().flatMap { area in area.tabs.compactMap { $0.content.pane?.id } }
            effects.paneIDsToRemove.append(contentsOf: paneIDs)
        }
        state.workspaceRoots.removeValue(forKey: key)
        state.focusedAreaID.removeValue(forKey: key)
        state.focusHistory.removeValue(forKey: key)

        guard state.activeWorktreeID[projectID] == worktreeID else { return }
        if let replacement {
            state.activeWorktreeID[projectID] = replacement.id
            WorkspaceReducerShared.ensureWorkspaceExists(
                projectID: projectID,
                worktreeID: replacement.id,
                worktreePath: replacement.path,
                state: &state,
                effects: &effects
            )
            return
        }

        let hasProjectWorkspace = state.workspaceRoots.keys.contains { $0.projectID == projectID }
        if hasProjectWorkspace,
           let fallback = state.workspaceRoots.keys
           .filter({ $0.projectID == projectID })
           .min(by: { $0.worktreeID.uuidString < $1.worktreeID.uuidString })
        {
            state.activeWorktreeID[projectID] = fallback.worktreeID
            return
        }

        state.activeWorktreeID.removeValue(forKey: projectID)
        if state.activeProjectID == projectID {
            state.activeProjectID = nil
        }
    }

    static func cycleProject(
        projects: [Project],
        worktrees: [UUID: [Worktree]],
        forward: Bool,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) {
        guard projects.count > 1,
              let currentID = state.activeProjectID,
              let index = projects.firstIndex(where: { $0.id == currentID })
        else { return }
        let next = forward ? (index + 1) % projects.count : (index - 1 + projects.count) % projects.count
        let project = projects[next]
        let list = worktrees[project.id] ?? []
        let existingID = state.activeWorktreeID[project.id]
        let target = list.first(where: { $0.id == existingID })
            ?? list.first(where: { $0.isPrimary })
            ?? list.first
        guard let worktree = target else { return }
        state.activeProjectID = project.id
        state.activeWorktreeID[project.id] = worktree.id
        WorkspaceReducerShared.ensureWorkspaceExists(
            projectID: project.id,
            worktreeID: worktree.id,
            worktreePath: worktree.path,
            state: &state,
            effects: &effects
        )
    }
}
