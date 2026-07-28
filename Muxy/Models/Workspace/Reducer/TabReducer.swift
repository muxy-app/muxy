import Foundation

@MainActor
enum TabReducer {
    static func createTab(projectID: UUID, areaID: UUID?, state: inout WorkspaceState) -> UUID? {
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state) else { return nil }
        return createTab(key: key, areaID: areaID, state: &state)
    }

    static func createTab(key: WorktreeKey, areaID: UUID?, state: inout WorkspaceState) -> UUID? {
        guard let area = WorkspaceReducerShared.resolveArea(key: key, areaID: areaID, state: state) else { return nil }
        FocusReducer.focusArea(area.id, key: key, state: &state)
        let alongsideTabID = area.activeTab.map { $0.parentTabID ?? $0.id }
        let order = normalizedTopLevelOrder(key: key, state: state)
        let tabID = area.createTab()
        state.topLevelTabOrder[key] = order + [tabID]
        TopLevelTabReducer.registerTopLevelTab(
            tabID,
            alongside: alongsideTabID,
            key: key,
            state: &state
        )
        return tabID
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
        var order = normalizedTopLevelOrder(key: key, state: state)
        guard let createdTabID = area.createTabAdjacent(to: tabID, side: side) else { return }
        let targetIndex = order.firstIndex(of: tabID) ?? order.count
        let insertionIndex = side == .left ? targetIndex : min(targetIndex + 1, order.count)
        order.insert(createdTabID, at: insertionIndex)
        state.topLevelTabOrder[key] = order
        let alongsideTabID = area.tabs.first(where: { $0.id == tabID }).map { $0.parentTabID ?? $0.id }
        TopLevelTabReducer.registerTopLevelTab(
            createdTabID,
            alongside: alongsideTabID,
            key: key,
            state: &state
        )
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
        let alongsideTabID = area.activeTab.map { $0.parentTabID ?? $0.id }
        let order = normalizedTopLevelOrder(key: key, state: state)
        let tabID = area.createTab(inDirectory: directory)
        state.topLevelTabOrder[key] = order + [tabID]
        TopLevelTabReducer.registerTopLevelTab(
            tabID,
            alongside: alongsideTabID,
            key: key,
            state: &state
        )
        return tabID
    }

    static func createCommandTab(_ request: CommandTabRequest, state: inout WorkspaceState) -> UUID? {
        guard let key = WorkspaceReducerShared.activeKey(projectID: request.projectID, state: state),
              let area = WorkspaceReducerShared.resolveArea(key: key, areaID: request.areaID, state: state)
        else { return nil }
        FocusReducer.focusArea(area.id, key: key, state: &state)
        let alongsideTabID = area.activeTab.map { $0.parentTabID ?? $0.id }
        let order = normalizedTopLevelOrder(key: key, state: state)
        let tabID = area.createCommandTab(
            name: request.name,
            command: request.command,
            closesOnCommandExit: request.closesOnCommandExit,
            directory: request.directory
        )
        if let tabID {
            state.topLevelTabOrder[key] = order + [tabID]
            TopLevelTabReducer.registerTopLevelTab(
                tabID,
                alongside: alongsideTabID,
                key: key,
                state: &state
            )
        }
        return tabID
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
        let alongsideTabID = area.activeTab.map { $0.parentTabID ?? $0.id }
        let order = normalizedTopLevelOrder(key: key, state: state)
        let tabID = area.createExtensionTab(
            extensionID: request.extensionID,
            tabTypeID: request.tabTypeID,
            title: request.title,
            data: request.data
        )
        state.topLevelTabOrder[key] = order + [tabID]
        TopLevelTabReducer.registerTopLevelTab(
            tabID,
            alongside: alongsideTabID,
            key: key,
            state: &state
        )
        return tabID
    }

    static func createBrowserTab(
        projectID: UUID,
        areaID: UUID?,
        url: URL?,
        profileID: UUID,
        state: inout WorkspaceState
    ) -> UUID? {
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state) else { return nil }
        return createBrowserTab(key: key, areaID: areaID, url: url, profileID: profileID, state: &state)
    }

    static func createBrowserTab(
        key: WorktreeKey,
        areaID: UUID?,
        url: URL?,
        profileID: UUID,
        state: inout WorkspaceState
    ) -> UUID? {
        guard BrowserPreferences.isEnabled,
              let area = WorkspaceReducerShared.resolveArea(key: key, areaID: areaID, state: state)
        else { return nil }
        FocusReducer.focusArea(area.id, key: key, state: &state)
        let alongsideTabID = area.activeTab.map { $0.parentTabID ?? $0.id }
        let order = normalizedTopLevelOrder(key: key, state: state)
        let tabID = area.createBrowserTab(url: url, profileID: profileID)
        state.topLevelTabOrder[key] = order + [tabID]
        TopLevelTabReducer.registerTopLevelTab(
            tabID,
            alongside: alongsideTabID,
            key: key,
            state: &state
        )
        return tabID
    }

    static func selectTab(projectID: UUID, areaID: UUID?, tabID: UUID, state: inout WorkspaceState) {
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state) else { return }
        selectTab(key: key, areaID: areaID, tabID: tabID, state: &state)
    }

    static func selectTab(key: WorktreeKey, areaID: UUID?, tabID: UUID, state: inout WorkspaceState) {
        guard let area = WorkspaceReducerShared.resolveArea(key: key, areaID: areaID, state: state) else { return }
        FocusReducer.focusArea(area.id, key: key, state: &state)
        area.selectTab(tabID)
    }

    static func selectTabByIndex(projectID: UUID, index: Int, state: inout WorkspaceState) {
        guard index >= 0 else { return }
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state),
              let root = state.workspaceRoots[key]
        else { return }
        let tabs = root.topLevelTabs(order: normalizedTopLevelOrder(key: key, state: state))
        guard index < tabs.count else { return }
        let target = tabs[index]
        FocusReducer.focusArea(target.area.id, key: key, state: &state)
        target.area.selectTab(target.tab.id)
    }

    static func selectNextTab(projectID: UUID, state: inout WorkspaceState) {
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state) else { return }
        selectNextTab(key: key, state: &state)
    }

    static func selectNextTab(key: WorktreeKey, state: inout WorkspaceState) {
        selectRelativeTopLevelTab(key: key, offset: 1, state: &state)
    }

    static func selectPreviousTab(projectID: UUID, state: inout WorkspaceState) {
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state) else { return }
        selectPreviousTab(key: key, state: &state)
    }

    static func selectPreviousTab(key: WorktreeKey, state: inout WorkspaceState) {
        selectRelativeTopLevelTab(key: key, offset: -1, state: &state)
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

        guard let tab = area.tabs.first(where: { $0.id == tabID }),
              !tab.isPinned
        else { return }
        if tab.parentTabID == nil {
            closeTopLevelTab(tab, areaID: areaID, key: key, state: &state, effects: &effects)
            return
        }

        if let paneID = area.closeTab(tabID) {
            effects.paneIDsToRemove.append(paneID)
        }

        guard area.tabs.isEmpty else { return }
        SplitReducer.closeArea(areaID, key: key, state: &state, effects: &effects)
    }

    static func selectRelativeFlatTab(key: WorktreeKey, offset: Int, state: inout WorkspaceState) {
        guard let root = state.workspaceRoots[key],
              let focusedAreaID = state.focusedAreaID[key]
        else { return }
        let entries = root.flatTabLocations(topLevelOrder: state.topLevelTabOrder[key] ?? [])
        guard entries.count > 1,
              let focusedArea = root.findArea(id: focusedAreaID),
              let activeTabID = focusedArea.activeTabID,
              let index = entries.firstIndex(where: { $0.area.id == focusedAreaID && $0.tab.id == activeTabID })
        else { return }
        let target = entries[(index + offset + entries.count) % entries.count]
        FocusReducer.focusArea(target.area.id, key: key, state: &state)
        target.area.selectTab(target.tab.id)
    }

    private static func selectRelativeTopLevelTab(
        key: WorktreeKey,
        offset: Int,
        state: inout WorkspaceState
    ) {
        guard let root = state.workspaceRoots[key],
              let focusedAreaID = state.focusedAreaID[key],
              let focusedTab = root.findArea(id: focusedAreaID)?.activeTab
        else { return }
        let topLevelTabID = focusedTab.parentTabID ?? focusedTab.id
        let tabs = root.topLevelTabs(order: normalizedTopLevelOrder(key: key, state: state))
        guard tabs.count > 1,
              let index = tabs.firstIndex(where: { $0.tab.id == topLevelTabID })
        else { return }
        let target = tabs[(index + offset + tabs.count) % tabs.count]
        FocusReducer.focusArea(target.area.id, key: key, state: &state)
        target.area.selectTab(target.tab.id)
    }

    private static func closeTopLevelTab(
        _ tab: TerminalTab,
        areaID: UUID,
        key: WorktreeKey,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) {
        guard var root = state.workspaceRoots[key] else { return }
        let originalOrder = normalizedTopLevelOrder(key: key, state: state)
        let closedIndex = originalOrder.firstIndex(of: tab.id) ?? 0
        let sourceGroupOrder = state.topLevelTabLayouts[key]?
            .group(containingTabID: tab.id)?
            .tabIDs
            ?? originalOrder
        let sourceGroupClosedIndex = sourceGroupOrder.firstIndex(of: tab.id) ?? 0
        let preservesEmptyWorkspace = state.keepProjectOpenWhenEmpty
            && root.allTabs().count(where: { $0.parentTabID == nil }) == 1
        let ownedIDs = Set(root.allTabs().filter { $0.parentTabID == tab.id }.map(\.id) + [tab.id])
        let closedTabWasFocused = state.focusedAreaID[key]
            .flatMap(root.findArea(id:))?
            .activeTab
            .map { ($0.parentTabID ?? $0.id) == tab.id }
            ?? false
        let affectedAreas = root.allAreas().filter { area in
            area.tabs.contains { ownedIDs.contains($0.id) }
        }
        for affectedArea in affectedAreas {
            let ownedTabs = affectedArea.tabs.filter { ownedIDs.contains($0.id) }
            for ownedTab in ownedTabs {
                if let paneID = ownedTab.content.pane?.id {
                    effects.paneIDsToRemove.append(paneID)
                }
                _ = affectedArea.extractTabForMove(ownedTab.id)
            }
        }
        for emptyArea in affectedAreas where emptyArea.tabs.isEmpty {
            if preservesEmptyWorkspace, emptyArea.id == areaID {
                continue
            }
            guard let updated = root.removing(areaID: emptyArea.id) else {
                WorkspaceReducerShared.clearWorkspace(key: key, state: &state)
                WorkspaceReducerShared.handleProjectEmptiedIfNeeded(
                    projectID: key.projectID,
                    state: &state,
                    effects: &effects
                )
                return
            }
            root = updated
            state.focusHistory[key]?.removeAll { $0 == emptyArea.id }
        }
        state.workspaceRoots[key] = root
        state.topLevelTabOrder[key]?.removeAll { $0 == tab.id }
        let remainingRoots = root.topLevelTabs(order: normalizedTopLevelOrder(key: key, state: state))
        guard !remainingRoots.isEmpty else {
            if preservesEmptyWorkspace {
                state.focusedAreaID[key] = areaID
                return
            }
            WorkspaceReducerShared.clearWorkspace(key: key, state: &state)
            WorkspaceReducerShared.handleProjectEmptiedIfNeeded(
                projectID: key.projectID,
                state: &state,
                effects: &effects
            )
            return
        }
        if closedTabWasFocused {
            let remainingByID = Dictionary(uniqueKeysWithValues: remainingRoots.map { ($0.tab.id, $0) })
            let remainingSourceGroup = sourceGroupOrder.compactMap { remainingByID[$0] }
            let next = remainingSourceGroup.isEmpty
                ? remainingRoots[min(closedIndex, remainingRoots.count - 1)]
                : remainingSourceGroup[min(sourceGroupClosedIndex, remainingSourceGroup.count - 1)]
            state.focusedAreaID[key] = next.area.id
            next.area.selectTab(next.tab.id)
        }
    }

    private static func normalizedTopLevelOrder(key: WorktreeKey, state: WorkspaceState) -> [UUID] {
        guard let root = state.workspaceRoots[key] else { return [] }
        let rootIDs = root.allTabs().filter { $0.parentTabID == nil }.map(\.id)
        let persisted = state.topLevelTabOrder[key] ?? []
        return persisted.filter { rootIDs.contains($0) } + rootIDs.filter { !persisted.contains($0) }
    }
}
