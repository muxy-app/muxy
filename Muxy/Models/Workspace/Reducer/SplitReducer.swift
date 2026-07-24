import Foundation

@MainActor
enum SplitReducer {
    @discardableResult
    static func splitArea(_ request: AppState.SplitAreaRequest, state: inout WorkspaceState) -> UUID? {
        guard let key = WorkspaceReducerShared.activeKey(projectID: request.projectID, state: state) else { return nil }
        return splitArea(request, key: key, state: &state)
    }

    @discardableResult
    static func splitArea(
        _ request: AppState.SplitAreaRequest,
        key: WorktreeKey,
        state: inout WorkspaceState
    ) -> UUID? {
        guard let root = state.workspaceRoots[key] else { return nil }
        let (newRoot, newAreaID) = root.splitting(
            areaID: request.areaID,
            direction: request.direction,
            position: request.position,
            command: request.command
        )
        state.workspaceRoots[key] = newRoot
        guard let newAreaID else { return nil }
        FocusReducer.focusArea(newAreaID, key: key, state: &state)
        return newRoot.findArea(id: newAreaID)?.tabs.first?.content.pane?.id
    }

    static func splitBrowserArea(
        key: WorktreeKey,
        areaID: UUID,
        url: URL?,
        profileID: UUID,
        state: inout WorkspaceState
    ) -> UUID? {
        guard BrowserPreferences.isEnabled,
              let root = state.workspaceRoots[key],
              let sourceArea = root.findArea(id: areaID),
              let activeTab = sourceArea.activeTab
        else { return nil }
        let browserState = BrowserTabState(
            projectPath: sourceArea.projectPath,
            url: url,
            profileID: profileID
        )
        let tab = TerminalTab(
            browserState: browserState,
            parentTabID: activeTab.parentTabID ?? activeTab.id
        )
        let (newRoot, newAreaID) = root.splittingWithTab(
            areaID: areaID,
            direction: .horizontal,
            position: .second,
            tab: tab
        )
        guard let newAreaID else { return nil }
        state.workspaceRoots[key] = newRoot
        FocusReducer.focusArea(newAreaID, key: key, state: &state)
        return tab.id
    }

    static func closeArea(
        _ areaID: UUID,
        key: WorktreeKey,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) {
        let removed = removeAreaFromTree(areaID, key: key, state: &state, effects: &effects)
        guard !removed else { return }
        WorkspaceReducerShared.clearWorkspace(key: key, state: &state)
        WorkspaceReducerShared.handleProjectEmptiedIfNeeded(
            projectID: key.projectID,
            state: &state,
            effects: &effects
        )
    }

    static func moveTab(
        _ request: TabMoveRequest,
        key: WorktreeKey,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) {
        switch request {
        case let .toArea(tabID, sourceAreaID, destinationAreaID):
            guard sourceAreaID != destinationAreaID else { return }
            guard let root = state.workspaceRoots[key],
                  let sourceArea = root.findArea(id: sourceAreaID),
                  let destArea = root.findArea(id: destinationAreaID),
                  let sourceIndex = sourceArea.tabs.firstIndex(where: { $0.id == tabID })
            else { return }
            let tab = sourceArea.tabs[sourceIndex]
            let topLevelTabID = tab.parentTabID ?? tab.id
            guard let destinationIndex = destArea.tabs.firstIndex(where: {
                ($0.parentTabID ?? $0.id) == topLevelTabID
            })
            else { return }
            let destinationTab = destArea.tabs[destinationIndex]
            sourceArea.tabs[sourceIndex] = destinationTab
            destArea.tabs[destinationIndex] = tab
            if sourceArea.activeTabID == tab.id {
                sourceArea.activeTabID = destinationTab.id
            }
            if destArea.activeTabID == destinationTab.id {
                destArea.activeTabID = tab.id
            }
            FocusReducer.focusArea(destinationAreaID, key: key, state: &state)

        case let .toNewSplit(tabID, sourceAreaID, targetAreaID, split):
            guard let root = state.workspaceRoots[key],
                  let sourceArea = root.findArea(id: sourceAreaID),
                  let tab = sourceArea.tabs.first(where: { $0.id == tabID }),
                  let targetArea = root.findArea(id: targetAreaID)
            else { return }
            let topLevelTabID = tab.parentTabID ?? tab.id
            guard targetArea.tabs.contains(where: { ($0.parentTabID ?? $0.id) == topLevelTabID }),
                  let movedTab = sourceArea.extractTabForMove(tabID)
            else { return }

            let shouldCollapseSource = sourceArea.tabs.isEmpty
            let (newRoot, newAreaID) = root.splittingWithTab(
                areaID: targetAreaID,
                direction: split.direction,
                position: split.position,
                tab: movedTab
            )
            state.workspaceRoots[key] = newRoot

            if let newAreaID {
                FocusReducer.focusArea(newAreaID, key: key, state: &state)
            }

            guard shouldCollapseSource else { return }
            let collapseAreaID = (sourceAreaID == targetAreaID) ? targetAreaID : sourceAreaID
            effects.deferredAreaCollapses.append(.init(key: key, areaID: collapseAreaID))
        }
    }

    private static func collapseEmptyArea(
        _ areaID: UUID,
        key: WorktreeKey,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) {
        _ = removeAreaFromTree(areaID, key: key, state: &state, effects: &effects)
    }

    @discardableResult
    private static func removeAreaFromTree(
        _ areaID: UUID,
        key: WorktreeKey,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) -> Bool {
        guard let root = state.workspaceRoots[key] else { return false }
        if let area = root.findArea(id: areaID) {
            effects.paneIDsToRemove.append(contentsOf: area.tabs.compactMap { $0.content.pane?.id })
        }
        guard let newRoot = root.removing(areaID: areaID) else { return false }
        state.workspaceRoots[key] = newRoot
        state.focusHistory[key]?.removeAll { $0 == areaID }
        guard state.focusedAreaID[key] == areaID else { return true }
        let remaining = newRoot.allAreas()
        let previousID = FocusReducer.popFocusHistory(key: key, validAreas: remaining, state: &state)
        state.focusedAreaID[key] = previousID ?? remaining.first?.id
        return true
    }
}
