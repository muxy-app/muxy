import Foundation

@MainActor
struct WorkspaceState {
    var activeProjectID: UUID?
    var activeWorktreeID: [UUID: UUID]
    var workspaceRoots: [WorktreeKey: SplitNode]
    var focusedAreaID: [WorktreeKey: UUID]
    var focusHistory: [WorktreeKey: [UUID]]
}

@MainActor
struct WorkspaceSideEffects {
    var paneIDsToRemove: [UUID] = []
    var projectIDsToRemove: [UUID] = []
}

@MainActor
enum WorkspaceReducer {
    static func reduce(action: AppState.Action, state: inout WorkspaceState) -> WorkspaceSideEffects {
        var effects = WorkspaceSideEffects()

        switch action {
        case let .selectProject(projectID, worktreeID, worktreePath),
             let .selectWorktree(projectID, worktreeID, worktreePath):
            ProjectLifecycleReducer.selectProject(
                projectID: projectID,
                worktreeID: worktreeID,
                worktreePath: worktreePath,
                state: &state
            )

        case let .removeProject(projectID):
            ProjectLifecycleReducer.removeProject(projectID: projectID, state: &state, effects: &effects)

        case let .removeWorktree(projectID, worktreeID, replacementWorktreeID, replacementWorktreePath):
            let replacement: ProjectLifecycleReducer.WorktreeReplacement? =
                if let replacementWorktreeID, let replacementWorktreePath {
                    ProjectLifecycleReducer.WorktreeReplacement(
                        id: replacementWorktreeID,
                        path: replacementWorktreePath
                    )
                } else {
                    nil
                }
            ProjectLifecycleReducer.removeWorktree(
                projectID: projectID,
                worktreeID: worktreeID,
                replacement: replacement,
                state: &state,
                effects: &effects
            )

        case let .selectNextProject(projects, worktrees):
            ProjectLifecycleReducer.cycleProject(
                projects: projects,
                worktrees: worktrees,
                forward: true,
                state: &state
            )

        case let .selectPreviousProject(projects, worktrees):
            ProjectLifecycleReducer.cycleProject(
                projects: projects,
                worktrees: worktrees,
                forward: false,
                state: &state
            )

        case let .createTab(projectID, areaID):
            TabReducer.createTab(projectID: projectID, areaID: areaID, state: &state)

        case let .createVCSTab(projectID, areaID):
            TabReducer.createVCSTab(projectID: projectID, areaID: areaID, state: &state)

        case let .createEditorTab(projectID, areaID, filePath):
            TabReducer.createEditorTab(projectID: projectID, areaID: areaID, filePath: filePath, state: &state)

        case let .createExternalEditorTab(projectID, areaID, filePath, command):
            TabReducer.createExternalEditorTab(
                projectID: projectID,
                areaID: areaID,
                filePath: filePath,
                command: command,
                state: &state
            )

        case let .closeTab(projectID, areaID, tabID):
            guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state) else { break }
            TabReducer.closeTab(tabID, areaID: areaID, key: key, state: &state, effects: &effects)

        case let .selectTab(projectID, areaID, tabID):
            TabReducer.selectTab(projectID: projectID, areaID: areaID, tabID: tabID, state: &state)

        case let .selectTabByIndex(projectID, areaID, index):
            TabReducer.selectTabByIndex(projectID: projectID, areaID: areaID, index: index, state: &state)

        case let .selectNextTab(projectID):
            TabReducer.selectNextTab(projectID: projectID, state: state)

        case let .selectPreviousTab(projectID):
            TabReducer.selectPreviousTab(projectID: projectID, state: state)

        case let .splitArea(request):
            SplitReducer.splitArea(request, state: &state)

        case let .closeArea(projectID, areaID):
            guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state) else { break }
            SplitReducer.closeArea(areaID, key: key, state: &state, effects: &effects)

        case let .moveTab(projectID, request):
            guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state) else { break }
            SplitReducer.moveTab(request, key: key, state: &state, effects: &effects)

        case let .focusArea(projectID, areaID):
            FocusReducer.focusArea(projectID: projectID, areaID: areaID, state: &state)

        case let .focusPaneLeft(projectID):
            FocusReducer.focusPane(projectID: projectID, direction: .left, state: &state)

        case let .focusPaneRight(projectID):
            FocusReducer.focusPane(projectID: projectID, direction: .right, state: &state)

        case let .focusPaneUp(projectID):
            FocusReducer.focusPane(projectID: projectID, direction: .up, state: &state)

        case let .focusPaneDown(projectID):
            FocusReducer.focusPane(projectID: projectID, direction: .down, state: &state)
        }

        return effects
    }
}
