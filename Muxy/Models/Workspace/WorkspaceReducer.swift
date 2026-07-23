import Foundation

@MainActor
struct WorkspaceState {
    var activeProjectID: UUID?
    var activeWorktreeID: [UUID: UUID]
    var workspaceRoots: [WorktreeKey: SplitNode]
    var focusedAreaID: [WorktreeKey: UUID]
    var focusHistory: [WorktreeKey: [UUID]]
    var topLevelTabOrder: [WorktreeKey: [UUID]] = [:]
    var topLevelTabLayouts: [WorktreeKey: TopLevelTabNode] = [:]
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
    var createdTabID: UUID?
    var createdPaneID: UUID?
}

@MainActor
enum WorkspaceReducer {
    static func reduce(action: AppState.Action, state: inout WorkspaceState) -> WorkspaceSideEffects {
        var effects = WorkspaceSideEffects()
        let reconciliationKeysBefore = reconciliationKeys(for: action, state: state)

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
            effects.createdTabID = TabReducer.createTab(projectID: projectID, areaID: areaID, state: &state)

        case let .createTabAdjacent(projectID, areaID, tabID, side):
            TabReducer.createTabAdjacent(
                projectID: projectID,
                areaID: areaID,
                tabID: tabID,
                side: side,
                state: &state
            )

        case let .createTabInDirectory(projectID, areaID, directory):
            effects.createdTabID = TabReducer.createTabInDirectory(
                projectID: projectID,
                areaID: areaID,
                directory: directory,
                state: &state
            )

        case let .createCommandTab(request):
            effects.createdTabID = TabReducer.createCommandTab(request, state: &state)

        case let .createExtensionTab(projectID, areaID, request):
            effects.createdTabID = TabReducer.createExtensionTab(
                projectID: projectID,
                areaID: areaID,
                request: request,
                state: &state
            )

        case let .createBrowserTab(projectID, areaID, url, profileID):
            effects.createdTabID = TabReducer.createBrowserTab(
                projectID: projectID,
                areaID: areaID,
                url: url,
                profileID: profileID,
                state: &state
            )

        case let .createTabInWorktree(key, areaID):
            guard state.workspaceRoots[key] != nil else { break }
            effects.createdTabID = TabReducer.createTab(key: key, areaID: areaID, state: &state)

        case let .createBrowserTabInWorktree(key, areaID, url, profileID):
            guard state.workspaceRoots[key] != nil else { break }
            effects.createdTabID = TabReducer.createBrowserTab(
                key: key,
                areaID: areaID,
                url: url,
                profileID: profileID,
                state: &state
            )

        case let .createBrowserSplit(projectID, areaID, url, profileID):
            guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state) else { break }
            effects.createdTabID = SplitReducer.splitBrowserArea(
                key: key,
                areaID: areaID,
                url: url,
                profileID: profileID,
                state: &state
            )

        case let .createBrowserSplitInWorktree(key, areaID, url, profileID):
            effects.createdTabID = SplitReducer.splitBrowserArea(
                key: key,
                areaID: areaID,
                url: url,
                profileID: profileID,
                state: &state
            )

        case let .closeTab(projectID, areaID, tabID):
            guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state) else { break }
            TabReducer.closeTab(tabID, areaID: areaID, key: key, state: &state, effects: &effects)

        case let .closeTabInWorktree(key, areaID, tabID):
            guard state.workspaceRoots[key] != nil else { break }
            TabReducer.closeTab(tabID, areaID: areaID, key: key, state: &state, effects: &effects)

        case let .selectTab(projectID, areaID, tabID):
            TabReducer.selectTab(projectID: projectID, areaID: areaID, tabID: tabID, state: &state)

        case let .selectTabInWorktree(key, areaID, tabID):
            guard state.workspaceRoots[key] != nil else { break }
            TabReducer.selectTab(key: key, areaID: areaID, tabID: tabID, state: &state)

        case let .selectTabByIndex(projectID, index):
            TabReducer.selectTabByIndex(projectID: projectID, index: index, state: &state)

        case let .selectNextTab(projectID):
            TabReducer.selectNextTab(projectID: projectID, state: &state)

        case let .selectPreviousTab(projectID):
            TabReducer.selectPreviousTab(projectID: projectID, state: &state)

        case let .selectNextTabInWorktree(key):
            guard state.workspaceRoots[key] != nil else { break }
            TabReducer.selectNextTab(key: key, state: &state)

        case let .selectPreviousTabInWorktree(key):
            guard state.workspaceRoots[key] != nil else { break }
            TabReducer.selectPreviousTab(key: key, state: &state)

        case let .selectNextFlatTabInWorktree(key):
            guard state.workspaceRoots[key] != nil else { break }
            TabReducer.selectRelativeFlatTab(key: key, offset: 1, state: &state)

        case let .selectPreviousFlatTabInWorktree(key):
            guard state.workspaceRoots[key] != nil else { break }
            TabReducer.selectRelativeFlatTab(key: key, offset: -1, state: &state)

        case let .splitArea(request):
            effects.createdPaneID = SplitReducer.splitArea(request, state: &state)

        case let .splitAreaInWorktree(key, request):
            guard state.workspaceRoots[key] != nil else { break }
            effects.createdPaneID = SplitReducer.splitArea(request, key: key, state: &state)

        case let .closeArea(projectID, areaID):
            guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state) else { break }
            closeArea(areaID, key: key, state: &state, effects: &effects)

        case let .closeAreaInWorktree(key, areaID):
            closeArea(areaID, key: key, state: &state, effects: &effects)

        case let .moveTab(projectID, request):
            guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state) else { break }
            SplitReducer.moveTab(request, key: key, state: &state, effects: &effects)

        case let .moveTopLevelTab(projectID, request):
            guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state) else { break }
            TopLevelTabReducer.moveTab(request, key: key, state: &state)

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

        case let .movePaneLeft(projectID):
            FocusReducer.movePane(projectID: projectID, direction: .left, state: &state)

        case let .movePaneRight(projectID):
            FocusReducer.movePane(projectID: projectID, direction: .right, state: &state)

        case let .movePaneUp(projectID):
            FocusReducer.movePane(projectID: projectID, direction: .up, state: &state)

        case let .movePaneDown(projectID):
            FocusReducer.movePane(projectID: projectID, direction: .down, state: &state)

        case let .cycleNextTabAcrossPanes(projectID):
            FocusReducer.cycleTabAcrossPanes(projectID: projectID, forward: true, state: &state)

        case let .cyclePreviousTabAcrossPanes(projectID):
            FocusReducer.cycleTabAcrossPanes(projectID: projectID, forward: false, state: &state)

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

        let reconciliationKeysAfter = reconciliationKeys(for: action, state: state)
        for key in reconciliationKeysBefore.union(reconciliationKeysAfter) {
            TopLevelTabReducer.reconcile(key: key, state: &state)
        }
        return effects
    }

    private static func reconciliationKeys(
        for action: AppState.Action,
        state: WorkspaceState
    ) -> Set<WorktreeKey> {
        switch action {
        case let .selectProject(projectID, worktreeID, _),
             let .selectWorktree(projectID, worktreeID, _):
            return [WorktreeKey(projectID: projectID, worktreeID: worktreeID)]

        case let .removeProject(projectID):
            return Set(state.workspaceRoots.keys.filter { $0.projectID == projectID })

        case let .removeWorktree(projectID, worktreeID, replacementWorktreeID, replacementWorktreePath):
            var keys: Set<WorktreeKey> = [WorktreeKey(projectID: projectID, worktreeID: worktreeID)]
            if let replacementWorktreeID, replacementWorktreePath != nil {
                keys.insert(WorktreeKey(projectID: projectID, worktreeID: replacementWorktreeID))
            }
            return keys

        case .selectNextProject,
             .selectPreviousProject:
            guard let projectID = state.activeProjectID,
                  let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state)
            else { return [] }
            return [key]

        case let .createTab(projectID, _),
             let .createTabAdjacent(projectID, _, _, _),
             let .createTabInDirectory(projectID, _, _),
             let .createExtensionTab(projectID, _, _),
             let .createBrowserTab(projectID, _, _, _),
             let .createBrowserSplit(projectID, _, _, _),
             let .closeTab(projectID, _, _),
             let .selectTab(projectID, _, _),
             let .selectTabByIndex(projectID, _),
             let .selectNextTab(projectID),
             let .selectPreviousTab(projectID),
             let .closeArea(projectID, _),
             let .moveTab(projectID, _),
             let .moveTopLevelTab(projectID, _),
             let .focusArea(projectID, _),
             let .focusPaneLeft(projectID),
             let .focusPaneRight(projectID),
             let .focusPaneUp(projectID),
             let .focusPaneDown(projectID),
             let .movePaneLeft(projectID),
             let .movePaneRight(projectID),
             let .movePaneUp(projectID),
             let .movePaneDown(projectID),
             let .cycleNextTabAcrossPanes(projectID),
             let .cyclePreviousTabAcrossPanes(projectID),
             let .applyLayout(projectID, _, _):
            guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state) else { return [] }
            return [key]

        case let .createCommandTab(request):
            guard let key = WorkspaceReducerShared.activeKey(projectID: request.projectID, state: state) else {
                return []
            }
            return [key]

        case let .splitArea(request):
            guard let key = WorkspaceReducerShared.activeKey(projectID: request.projectID, state: state) else {
                return []
            }
            return [key]

        case let .createTabInWorktree(key, _),
             let .createBrowserTabInWorktree(key, _, _, _),
             let .createBrowserSplitInWorktree(key, _, _, _),
             let .closeTabInWorktree(key, _, _),
             let .selectTabInWorktree(key, _, _),
             let .selectNextTabInWorktree(key),
             let .selectPreviousTabInWorktree(key),
             let .selectNextFlatTabInWorktree(key),
             let .selectPreviousFlatTabInWorktree(key),
             let .splitAreaInWorktree(key, _),
             let .closeAreaInWorktree(key, _):
            return [key]

        case let .navigate(projectID, worktreeID, _, _):
            return [WorktreeKey(projectID: projectID, worktreeID: worktreeID)]
        }
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
        state.topLevelTabOrder[key] = built.root.topLevelTabs().map(\.tab.id)
        state.topLevelTabLayouts[key] = .group(TopLevelTabGroup(
            tabIDs: state.topLevelTabOrder[key] ?? [],
            activeTabID: built.root.findArea(id: built.focusedAreaID)?.activeTab.map { $0.parentTabID ?? $0.id }
        ))
        state.focusHistory.removeValue(forKey: key)
    }

    private static func closeArea(
        _ areaID: UUID,
        key: WorktreeKey,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) {
        guard let area = state.workspaceRoots[key]?.findArea(id: areaID) else { return }
        if let tabID = area.activeTabID {
            TabReducer.closeTab(tabID, areaID: areaID, key: key, state: &state, effects: &effects)
        } else {
            SplitReducer.closeArea(areaID, key: key, state: &state, effects: &effects)
        }
    }
}
