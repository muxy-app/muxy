import Foundation

@MainActor
enum TabReducer {
    static func createTab(projectID: UUID, areaID: UUID?, state: inout WorkspaceState) -> UUID? {
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state),
              let area = WorkspaceReducerShared.resolveArea(key: key, areaID: areaID, state: state)
        else { return nil }
        FocusReducer.focusArea(area.id, key: key, state: &state)
        return area.createTab()
    }

    static func createTabAdjacent(
        projectID: UUID,
        areaID: UUID,
        tabID: UUID,
        side: TabArea.InsertSide,
        state: inout WorkspaceState
    ) {
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state),
              let area = WorkspaceReducerShared.resolveArea(key: key, areaID: areaID, state: state)
        else { return }
        FocusReducer.focusArea(area.id, key: key, state: &state)
        area.createTabAdjacent(to: tabID, side: side)
    }

    static func createTabInDirectory(
        projectID: UUID,
        areaID: UUID?,
        directory: String,
        state: inout WorkspaceState
    ) -> UUID? {
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state),
              let area = WorkspaceReducerShared.resolveArea(key: key, areaID: areaID, state: state)
        else { return nil }
        FocusReducer.focusArea(area.id, key: key, state: &state)
        return area.createTab(inDirectory: directory)
    }

    static func createCommandTab(_ request: CommandTabRequest, state: inout WorkspaceState) -> UUID? {
        guard let key = WorkspaceReducerShared.activeKey(projectID: request.projectID, state: state),
              let area = WorkspaceReducerShared.resolveArea(key: key, areaID: request.areaID, state: state)
        else { return nil }
        FocusReducer.focusArea(area.id, key: key, state: &state)
        return area.createCommandTab(
            name: request.name,
            command: request.command,
            closesOnCommandExit: request.closesOnCommandExit,
            directory: request.directory
        )
    }

    static func createExtensionTab(
        projectID: UUID,
        areaID: UUID?,
        request: AppState.CreateExtensionTabRequest,
        state: inout WorkspaceState
    ) -> UUID? {
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state),
              let root = state.workspaceRoots[key],
              let area = WorkspaceReducerShared.resolveArea(key: key, areaID: areaID, state: state)
        else { return nil }
        if request.singleton {
            for existingArea in root.allAreas() {
                guard let existing = existingArea.findExtensionTab(
                    extensionID: request.extensionID,
                    tabTypeID: request.tabTypeID
                )
                else { continue }
                existing.content.extensionState?.data = request.data
                FocusReducer.focusArea(existingArea.id, key: key, state: &state)
                existingArea.selectTab(existing.id)
                return existing.content.extensionState?.id
            }
        }
        FocusReducer.focusArea(area.id, key: key, state: &state)
        return area.createExtensionTab(
            extensionID: request.extensionID,
            tabTypeID: request.tabTypeID,
            title: request.title,
            data: request.data
        )
    }

    static func createBrowserTab(
        projectID: UUID,
        areaID: UUID?,
        url: URL?,
        profileID: UUID,
        state: inout WorkspaceState
    ) -> UUID? {
        guard BrowserPreferences.isEnabled,
              let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state),
              let area = WorkspaceReducerShared.resolveArea(key: key, areaID: areaID, state: state)
        else { return nil }
        FocusReducer.focusArea(area.id, key: key, state: &state)
        return area.createBrowserTab(url: url, profileID: profileID)
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
        guard !state.keepProjectOpenWhenEmpty else { return }
        WorkspaceReducerShared.clearWorkspace(key: key, state: &state)
        WorkspaceReducerShared.handleProjectEmptiedIfNeeded(
            projectID: key.projectID,
            state: &state,
            effects: &effects
        )
    }
}
