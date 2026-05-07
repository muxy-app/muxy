import Foundation

@MainActor
struct WorkspaceState {
    var activeProjectID: UUID?
    var activeWorktreeID: [UUID: UUID]
    var workspaceRoots: [WorktreeKey: SplitNode]
    var focusedAreaID: [WorktreeKey: UUID]
    var focusHistory: [WorktreeKey: [UUID]]
    var keepProjectOpenWhenEmpty: Bool = false
}

@MainActor
struct WorkspaceSideEffects {
    struct DeferredAreaCollapse {
        let key: WorktreeKey
        let areaID: UUID
    }

    var paneIDsToRemove: [UUID] = []
    var projectIDsToRemove: [UUID] = []
    var deferredAreaCollapses: [DeferredAreaCollapse] = []
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
                state: &state,
                effects: &effects
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
                state: &state,
                effects: &effects
            )

        case let .selectPreviousProject(projects, worktrees):
            ProjectLifecycleReducer.cycleProject(
                projects: projects,
                worktrees: worktrees,
                forward: false,
                state: &state,
                effects: &effects
            )

        case let .createTab(projectID, areaID):
            TabReducer.createTab(projectID: projectID, areaID: areaID, state: &state)

        case let .createTabInDirectory(projectID, areaID, directory):
            TabReducer.createTabInDirectory(
                projectID: projectID,
                areaID: areaID,
                directory: directory,
                state: &state
            )

        case let .createCommandTab(projectID, areaID, name, command):
            TabReducer.createCommandTab(
                projectID: projectID,
                areaID: areaID,
                name: name,
                command: command,
                state: &state
            )

        case let .createVCSTab(projectID, areaID):
            TabReducer.createVCSTab(projectID: projectID, areaID: areaID, state: &state)

        case let .createEditorTab(projectID, areaID, filePath, suppressInitialFocus):
            TabReducer.createEditorTab(
                projectID: projectID,
                areaID: areaID,
                filePath: filePath,
                suppressInitialFocus: suppressInitialFocus,
                state: &state
            )

        case let .createExternalEditorTab(projectID, areaID, filePath, command):
            TabReducer.createExternalEditorTab(
                projectID: projectID,
                areaID: areaID,
                filePath: filePath,
                command: command,
                state: &state
            )

        case let .createDiffViewerTab(projectID, areaID, request):
            TabReducer.createDiffViewerTab(
                projectID: projectID,
                areaID: areaID,
                request: request,
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

        case let .applyLayout(projectID, worktreePath, config):
            applyLayout(
                projectID: projectID,
                worktreePath: worktreePath,
                config: config,
                state: &state,
                effects: &effects
            )

        case let .navigate(projectID, worktreeID, areaID, tabID):
            let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
            guard state.workspaceRoots[key] != nil else { break }
            state.activeProjectID = projectID
            state.activeWorktreeID[projectID] = worktreeID
            if let tabID {
                TabReducer.selectTab(projectID: projectID, areaID: areaID, tabID: tabID, state: &state)
            } else {
                FocusReducer.focusArea(projectID: projectID, areaID: areaID, state: &state)
            }
        }

        return effects
    }

    private static func applyLayout(
        projectID: UUID,
        worktreePath: String,
        config: LayoutConfig,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) {
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state) else { return }
        guard let built = LayoutWorkspaceBuilder.build(config: config, projectPath: worktreePath) else { return }
        if let existingRoot = state.workspaceRoots[key] {
            let paneIDs = existingRoot.allAreas().flatMap { area in area.tabs.compactMap { $0.content.pane?.id } }
            effects.paneIDsToRemove.append(contentsOf: paneIDs)
        }
        state.workspaceRoots[key] = built.root
        state.focusedAreaID[key] = built.focusedAreaID
        state.focusHistory.removeValue(forKey: key)
    }
}
