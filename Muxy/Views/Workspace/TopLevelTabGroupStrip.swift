import SwiftUI

struct TopLevelTabGroupStrip: View {
    let project: Project
    let worktreeKey: WorktreeKey
    let groupID: UUID
    var isWindowTitleBar = false
    var showDevelopmentBadge = false
    var openProjectPath: String?

    @Environment(AppState.self) private var appState
    @Environment(BrowserProfileStore.self) private var browserProfileStore
    @AppStorage(BrowserPreferences.enabledKey) private var browserEnabled = true

    private var root: SplitNode? {
        appState.workspaceRoots[worktreeKey]
    }

    private var group: TopLevelTabGroup? {
        appState.topLevelTabLayouts[worktreeKey]?.group(id: groupID)
    }

    private var tabs: [(area: TabArea, tab: TerminalTab)] {
        appState.topLevelTabs(for: worktreeKey, groupID: groupID)
    }

    private var visibleLayout: VisiblePaneNode? {
        appState.visibleLayout(for: worktreeKey, groupID: groupID)
    }

    private var targetAreaID: UUID? {
        let panes = visibleLayout?.allPanes() ?? []
        if appState.activeTopLevelTabID(for: worktreeKey) == group?.activeTabID,
           let focusedAreaID = appState.focusedAreaID[worktreeKey],
           panes.contains(where: { $0.area.id == focusedAreaID })
        {
            return focusedAreaID
        }
        return panes.first?.area.id
    }

    private var maximizedAreaID: UUID? {
        guard let maximizedPane = appState.maximizedPanes[worktreeKey],
              maximizedPane.topLevelTabID == group?.activeTabID,
              visibleLayout?.allPanes().contains(where: { $0.area.id == maximizedPane.areaID }) == true
        else { return nil }
        return maximizedPane.areaID
    }

    var body: some View {
        if let root,
           let group,
           let firstAreaID = appState.tabStripAreaID(for: worktreeKey, groupID: groupID)
        {
            PaneTabStrip(
                areaID: firstAreaID,
                tabs: PaneTabStrip.snapshots(
                    from: tabs.map(\.tab),
                    including: root.allTabs()
                ),
                activeTabID: group.activeTabID,
                isFocused: appState.activeTopLevelTabID(for: worktreeKey) == group.activeTabID,
                isWindowTitleBar: isWindowTitleBar,
                showDevelopmentBadge: showDevelopmentBadge,
                openProjectPath: openProjectPath,
                projectID: project.id,
                shortcutIndicesByTabID: appState.topLevelTabShortcutIndices(for: worktreeKey),
                topLevelGroupID: groupID,
                onSelectTab: selectTab,
                onCreateTab: {
                    activateGroup()
                    appState.dispatch(.createTab(projectID: project.id, areaID: targetAreaID))
                },
                onOpenBrowser: browserEnabled ? {
                    activateGroup()
                    appState.dispatch(.createBrowserTab(
                        projectID: project.id,
                        areaID: targetAreaID,
                        url: BrowserURL.homeURL,
                        profileID: browserProfileStore.defaultProfileID
                    ))
                } : nil,
                onCloseTab: closeTab,
                onCloseOtherTabs: closeOtherTabs,
                onCloseTabsToLeft: closeTabsToLeft,
                onCloseTabsToRight: closeTabsToRight,
                onSplit: split,
                onDropAction: { result in
                    appState.dispatch(result.action(projectID: project.id))
                },
                showMaximizeButton: (visibleLayout?.allPanes().count ?? 0) > 1,
                isMaximized: maximizedAreaID != nil,
                onToggleMaximize: targetAreaID.map { areaID in
                    {
                        activateGroup()
                        appState.toggleMaximize(
                            areaID: areaID,
                            topLevelTabID: group.activeTabID,
                            for: project.id
                        )
                    }
                },
                onCreateTabAdjacent: createTabAdjacent,
                onTogglePin: { tabID in
                    appState.togglePinTopLevelTab(tabID, for: worktreeKey)
                },
                onSetCustomTitle: setCustomTitle,
                onSetColorID: setColorID,
                onReorderTab: { fromOffsets, toOffset in
                    appState.reorderTopLevelTabs(
                        for: worktreeKey,
                        groupID: groupID,
                        fromOffsets: fromOffsets,
                        toOffset: toOffset
                    )
                }
            )
        }
    }

    private func selectTab(_ tabID: UUID) {
        guard let located = root?.locateTab(id: tabID),
              appState.activeTopLevelTabID(for: worktreeKey) != tabID
        else { return }
        appState.dispatch(.selectTab(
            projectID: project.id,
            areaID: located.area.id,
            tabID: tabID
        ))
    }

    private func closeTab(_ tabID: UUID) {
        guard let areaID = root?.locateTab(id: tabID)?.area.id else { return }
        appState.closeTab(tabID, areaID: areaID, projectID: project.id)
    }

    private func closeOtherTabs(_ tabID: UUID) {
        for item in tabs where item.tab.id != tabID && !item.tab.isPinned {
            appState.closeTab(item.tab.id, areaID: item.area.id, projectID: project.id)
        }
    }

    private func closeTabsToLeft(_ tabID: UUID) {
        guard let index = tabs.firstIndex(where: { $0.tab.id == tabID }) else { return }
        for item in tabs.prefix(index) where !item.tab.isPinned {
            appState.closeTab(item.tab.id, areaID: item.area.id, projectID: project.id)
        }
    }

    private func closeTabsToRight(_ tabID: UUID) {
        guard let index = tabs.firstIndex(where: { $0.tab.id == tabID }) else { return }
        for item in tabs.suffix(from: index + 1) where !item.tab.isPinned {
            appState.closeTab(item.tab.id, areaID: item.area.id, projectID: project.id)
        }
    }

    private func split(_ direction: SplitDirection) {
        activateGroup()
        guard let targetAreaID else { return }
        appState.dispatch(.splitArea(.init(
            projectID: project.id,
            areaID: targetAreaID,
            direction: direction,
            position: .second
        )))
    }

    private func createTabAdjacent(_ tabID: UUID, side: TabArea.InsertSide) {
        selectTab(tabID)
        guard let areaID = root?.locateTab(id: tabID)?.area.id else { return }
        appState.dispatch(.createTabAdjacent(
            projectID: project.id,
            areaID: areaID,
            tabID: tabID,
            side: side
        ))
    }

    private func setCustomTitle(_ tabID: UUID, title: String?) {
        guard let area = root?.locateTab(id: tabID)?.area else { return }
        area.setCustomTitle(tabID, title: title)
        appState.saveWorkspaces()
    }

    private func setColorID(_ tabID: UUID, colorID: String?) {
        guard let area = root?.locateTab(id: tabID)?.area else { return }
        area.setColorID(tabID, colorID: colorID)
        appState.saveWorkspaces()
    }

    private func activateGroup() {
        guard let tabID = group?.activeTabID,
              appState.activeTopLevelTabID(for: worktreeKey) != tabID
        else { return }
        selectTab(tabID)
    }
}
