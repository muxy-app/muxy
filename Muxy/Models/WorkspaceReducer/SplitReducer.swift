import Foundation

@MainActor
enum SplitReducer {
    static func splitArea(_ request: AppState.SplitAreaRequest, state: inout WorkspaceState) {
        guard let key = WorkspaceReducerShared.activeKey(projectID: request.projectID, state: state),
              let root = state.workspaceRoots[key]
        else { return }
        let (newRoot, newAreaID) = root.splitting(
            areaID: request.areaID,
            direction: request.direction,
            position: request.position
        )
        state.workspaceRoots[key] = newRoot
        guard let newAreaID else { return }
        FocusReducer.focusArea(newAreaID, key: key, state: &state)
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
                  let tab = sourceArea.removeTab(tabID)
            else { return }

            destArea.insertExistingTab(tab)
            FocusReducer.focusArea(destinationAreaID, key: key, state: &state)

            guard sourceArea.tabs.isEmpty else { return }
            effects.deferredAreaCollapses.append(.init(key: key, areaID: sourceAreaID))

        case let .toNewSplit(tabID, sourceAreaID, targetAreaID, split):
            guard let root = state.workspaceRoots[key],
                  let sourceArea = root.findArea(id: sourceAreaID),
                  let tab = sourceArea.removeTab(tabID)
            else { return }

            let shouldCollapseSource = sourceArea.tabs.isEmpty
            let (newRoot, newAreaID) = root.splittingWithTab(
                areaID: targetAreaID,
                direction: split.direction,
                position: split.position,
                tab: tab
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
