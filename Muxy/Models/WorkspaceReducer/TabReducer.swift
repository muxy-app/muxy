import Foundation

@MainActor
enum TabReducer {
    static func createTab(projectID: UUID, areaID: UUID?, state: inout WorkspaceState) {
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state),
              let area = WorkspaceReducerShared.resolveArea(key: key, areaID: areaID, state: state)
        else { return }
        FocusReducer.focusArea(area.id, key: key, state: &state)
        area.createTab()
    }

    static func createTabInDirectory(
        projectID: UUID,
        areaID: UUID?,
        directory: String,
        state: inout WorkspaceState
    ) {
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state),
              let area = WorkspaceReducerShared.resolveArea(key: key, areaID: areaID, state: state)
        else { return }
        FocusReducer.focusArea(area.id, key: key, state: &state)
        area.createTab(inDirectory: directory)
    }

    static func createCommandTab(
        projectID: UUID,
        areaID: UUID?,
        name: String,
        command: String,
        state: inout WorkspaceState
    ) {
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state),
              let area = WorkspaceReducerShared.resolveArea(key: key, areaID: areaID, state: state)
        else { return }
        FocusReducer.focusArea(area.id, key: key, state: &state)
        area.createCommandTab(name: name, command: command)
    }

    static func createVCSTab(projectID: UUID, areaID: UUID?, state: inout WorkspaceState) {
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state),
              let area = WorkspaceReducerShared.resolveArea(key: key, areaID: areaID, state: state)
        else { return }
        FocusReducer.focusArea(area.id, key: key, state: &state)
        area.createVCSTab()
    }

    static func createEditorTab(
        projectID: UUID,
        areaID: UUID?,
        filePath: String,
        suppressInitialFocus: Bool,
        state: inout WorkspaceState
    ) {
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state),
              let area = WorkspaceReducerShared.resolveArea(key: key, areaID: areaID, state: state)
        else { return }
        FocusReducer.focusArea(area.id, key: key, state: &state)
        area.createEditorTab(filePath: filePath, suppressInitialFocus: suppressInitialFocus)
    }

    static func createExternalEditorTab(
        projectID: UUID,
        areaID: UUID?,
        filePath: String,
        command: String,
        state: inout WorkspaceState
    ) {
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state),
              let area = WorkspaceReducerShared.resolveArea(key: key, areaID: areaID, state: state)
        else { return }
        FocusReducer.focusArea(area.id, key: key, state: &state)
        area.createExternalEditorTab(filePath: filePath, command: command)
    }

    static func createDiffViewerTab(
        projectID: UUID,
        areaID: UUID?,
        request: AppState.DiffViewerRequest,
        state: inout WorkspaceState
    ) {
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state),
              let area = WorkspaceReducerShared.resolveArea(key: key, areaID: areaID, state: state)
        else { return }
        FocusReducer.focusArea(area.id, key: key, state: &state)
        area.createDiffViewerTab(
            vcs: request.vcs,
            filePath: request.filePath,
            isStaged: request.isStaged
        )
    }

    static func selectTab(projectID: UUID, areaID: UUID?, tabID: UUID, state: inout WorkspaceState) {
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state),
              let area = WorkspaceReducerShared.resolveArea(key: key, areaID: areaID, state: state)
        else { return }
        FocusReducer.focusArea(area.id, key: key, state: &state)
        area.selectTab(tabID)
    }

    static func selectTabByIndex(projectID: UUID, index: Int, state: inout WorkspaceState) {
        guard index >= 0 else { return }
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state),
              let root = state.workspaceRoots[key]
        else { return }
        var remaining = index
        for area in root.allAreas() {
            guard remaining < area.tabs.count else {
                remaining -= area.tabs.count
                continue
            }
            let tab = area.tabs[remaining]
            FocusReducer.focusArea(area.id, key: key, state: &state)
            area.selectTab(tab.id)
            return
        }
    }

    static func selectNextTab(projectID: UUID, state: WorkspaceState) {
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state),
              let area = WorkspaceReducerShared.resolveArea(key: key, areaID: nil, state: state)
        else { return }
        area.selectNextTab()
    }

    static func selectPreviousTab(projectID: UUID, state: WorkspaceState) {
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state),
              let area = WorkspaceReducerShared.resolveArea(key: key, areaID: nil, state: state)
        else { return }
        area.selectPreviousTab()
    }

    static func closeTab(
        _ tabID: UUID,
        areaID: UUID,
        key: WorktreeKey,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) {
        guard let root = state.workspaceRoots[key],
              let area = root.findArea(id: areaID)
        else { return }

        let areaCount = root.allAreas().count
        if area.tabs.count <= 1, areaCount > 1 {
            SplitReducer.closeArea(areaID, key: key, state: &state, effects: &effects)
            return
        }

        if let paneID = area.closeTab(tabID) {
            effects.paneIDsToRemove.append(paneID)
        }

        guard area.tabs.isEmpty else { return }
        WorkspaceReducerShared.clearWorkspace(key: key, state: &state)
        WorkspaceReducerShared.handleProjectEmptiedIfNeeded(
            projectID: key.projectID,
            state: &state,
            effects: &effects
        )
    }
}
