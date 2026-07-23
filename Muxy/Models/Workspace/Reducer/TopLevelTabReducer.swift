import Foundation

@MainActor
enum TopLevelTabReducer {
    static func registerTopLevelTab(
        _ tabID: UUID,
        alongside topLevelTabID: UUID?,
        key: WorktreeKey,
        state: inout WorkspaceState
    ) {
        guard let layout = state.topLevelTabLayouts[key] else { return }
        let destination = topLevelTabID.flatMap(layout.group(containingTabID:))
            ?? layout.allGroups().first
        guard let destination,
              !layout.flattenedTabIDs().contains(tabID)
        else { return }
        destination.tabIDs.append(tabID)
        let positions = Dictionary(uniqueKeysWithValues: (state.topLevelTabOrder[key] ?? [])
            .enumerated().map { ($1, $0) })
        destination.tabIDs.sort {
            (positions[$0] ?? positions.count) < (positions[$1] ?? positions.count)
        }
        destination.activeTabID = tabID
    }

    static func moveTab(
        _ request: TopLevelTabMoveRequest,
        key: WorktreeKey,
        state: inout WorkspaceState
    ) {
        guard let root = state.workspaceRoots[key],
              var layout = state.topLevelTabLayouts[key]
        else { return }

        let tabID: UUID
        let sourceGroupID: UUID

        switch request {
        case let .toGroup(requestTabID, requestSourceGroupID, destinationGroupID):
            tabID = requestTabID
            sourceGroupID = requestSourceGroupID
            guard sourceGroupID != destinationGroupID,
                  let sourceGroup = layout.group(id: sourceGroupID),
                  let destinationGroup = layout.group(id: destinationGroupID),
                  sourceGroup.tabIDs.contains(tabID),
                  root.locateTab(id: tabID)?.tab.parentTabID == nil
            else { return }

            sourceGroup.tabIDs.removeAll { $0 == tabID }
            destinationGroup.tabIDs.append(tabID)
            destinationGroup.tabIDs = pinnedFirst(destinationGroup.tabIDs, root: root)
            destinationGroup.activeTabID = tabID
            if sourceGroup.activeTabID == tabID {
                sourceGroup.activeTabID = sourceGroup.tabIDs.first
            }
            if sourceGroup.tabIDs.isEmpty,
               let updated = layout.removingGroup(id: sourceGroupID)
            {
                layout = updated
            }

        case let .toNewSplit(requestTabID, requestSourceGroupID, targetGroupID, split):
            tabID = requestTabID
            sourceGroupID = requestSourceGroupID
            guard let sourceGroup = layout.group(id: sourceGroupID),
                  layout.group(id: targetGroupID) != nil,
                  sourceGroup.tabIDs.contains(tabID),
                  root.locateTab(id: tabID)?.tab.parentTabID == nil
            else { return }

            let newGroup: TopLevelTabGroup
            if sourceGroupID == targetGroupID {
                guard sourceGroup.tabIDs.count > 1 else { return }
                sourceGroup.tabIDs.removeAll { $0 == tabID }
                if sourceGroup.activeTabID == tabID {
                    sourceGroup.activeTabID = sourceGroup.tabIDs.first
                }
                newGroup = TopLevelTabGroup(tabIDs: [tabID], activeTabID: tabID)
            } else if sourceGroup.tabIDs.count == 1 {
                guard let updated = layout.removingGroup(id: sourceGroupID) else { return }
                layout = updated
                newGroup = sourceGroup
            } else {
                sourceGroup.tabIDs.removeAll { $0 == tabID }
                if sourceGroup.activeTabID == tabID {
                    sourceGroup.activeTabID = sourceGroup.tabIDs.first
                }
                newGroup = TopLevelTabGroup(tabIDs: [tabID], activeTabID: tabID)
            }

            layout = layout.insertingSplit(
                aroundGroupID: targetGroupID,
                newGroup: newGroup,
                placement: split
            )
        }

        state.topLevelTabLayouts[key] = layout
        if let located = root.locateTab(id: tabID) {
            FocusReducer.focusArea(located.area.id, key: key, state: &state)
            located.area.selectTab(tabID)
        }
    }

    static func reconcile(key: WorktreeKey, state: inout WorkspaceState) {
        guard let root = state.workspaceRoots[key] else {
            state.topLevelTabLayouts.removeValue(forKey: key)
            return
        }

        let orderedTabs = root.topLevelTabs(order: state.topLevelTabOrder[key] ?? [])
        let orderedIDs = orderedTabs.map(\.tab.id)
        let validIDs = Set(orderedIDs)
        guard !orderedIDs.isEmpty else {
            state.topLevelTabLayouts.removeValue(forKey: key)
            return
        }

        let layout = state.topLevelTabLayouts[key]?.pruningTabs(validTabIDs: validIDs)
            ?? .group(TopLevelTabGroup(tabIDs: orderedIDs, activeTabID: orderedIDs.first))

        let assignedIDs = Set(layout.flattenedTabIDs())
        for tabID in orderedIDs where !assignedIDs.contains(tabID) {
            let destination = destinationGroup(
                for: tabID,
                root: root,
                layout: layout,
                focusedAreaID: state.focusedAreaID[key]
            )
            destination.tabIDs.append(tabID)
            destination.activeTabID = tabID
        }

        if let focusedAreaID = state.focusedAreaID[key],
           let activeTab = root.findArea(id: focusedAreaID)?.activeTab,
           let group = layout.group(containingTabID: activeTab.parentTabID ?? activeTab.id)
        {
            group.activeTabID = activeTab.parentTabID ?? activeTab.id
        }

        state.topLevelTabLayouts[key] = layout
        state.topLevelTabOrder[key] = orderedIDs
    }

    private static func destinationGroup(
        for tabID: UUID,
        root: SplitNode,
        layout: TopLevelTabNode,
        focusedAreaID: UUID?
    ) -> TopLevelTabGroup {
        if let area = root.locateTab(id: tabID)?.area {
            for sibling in area.tabs where sibling.id != tabID {
                let siblingTopLevelID = sibling.parentTabID ?? sibling.id
                if let group = layout.group(containingTabID: siblingTopLevelID) {
                    return group
                }
            }
        }
        if let focusedAreaID,
           let focusedTab = root.findArea(id: focusedAreaID)?.activeTab,
           let group = layout.group(containingTabID: focusedTab.parentTabID ?? focusedTab.id)
        {
            return group
        }
        return layout.allGroups()[0]
    }

    private static func pinnedFirst(_ tabIDs: [UUID], root: SplitNode) -> [UUID] {
        let tabsByID = Dictionary(uniqueKeysWithValues: root.allTabs().map { ($0.id, $0) })
        return tabIDs.filter { tabsByID[$0]?.isPinned == true }
            + tabIDs.filter { tabsByID[$0]?.isPinned != true }
    }
}
