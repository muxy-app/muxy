import Foundation
import os
import SwiftUI

private let logger = Logger(subsystem: "app.muxy", category: "AppState")

@MainActor
@Observable
final class AppState {
    struct SplitAreaRequest {
        let projectID: UUID
        let areaID: UUID
        let direction: SplitDirection
        let position: SplitPosition
        var command: String?
    }

    struct CreateExtensionTabRequest {
        let extensionID: String
        let tabTypeID: String
        let title: String
        let data: ExtensionJSON?
        let singleton: Bool
    }

    enum Action {
        case selectProject(projectID: UUID, worktreeID: UUID, worktreePath: String)
        case selectWorktree(projectID: UUID, worktreeID: UUID, worktreePath: String)
        case removeProject(projectID: UUID)
        case removeWorktree(
            projectID: UUID,
            worktreeID: UUID,
            replacementWorktreeID: UUID?,
            replacementWorktreePath: String?
        )
        case createTab(projectID: UUID, areaID: UUID?)
        case createTabAdjacent(projectID: UUID, areaID: UUID, tabID: UUID, side: TabArea.InsertSide)
        case createTabInDirectory(projectID: UUID, areaID: UUID?, directory: String)
        case createCommandTab(CommandTabRequest)
        case createExtensionTab(projectID: UUID, areaID: UUID?, request: CreateExtensionTabRequest)
        case createBrowserTab(projectID: UUID, areaID: UUID?, url: URL?, profileID: UUID)
        case createTabInWorktree(key: WorktreeKey, areaID: UUID?)
        case createBrowserTabInWorktree(key: WorktreeKey, areaID: UUID?, url: URL?, profileID: UUID)
        case createBrowserSplit(projectID: UUID, areaID: UUID, url: URL?, profileID: UUID)
        case createBrowserSplitInWorktree(key: WorktreeKey, areaID: UUID, url: URL?, profileID: UUID)
        case closeTab(projectID: UUID, areaID: UUID, tabID: UUID)
        case closeTabInWorktree(key: WorktreeKey, areaID: UUID, tabID: UUID)
        case selectTab(projectID: UUID, areaID: UUID, tabID: UUID)
        case selectTabInWorktree(key: WorktreeKey, areaID: UUID, tabID: UUID)
        case selectTabByIndex(projectID: UUID, index: Int)
        case selectNextTab(projectID: UUID)
        case selectPreviousTab(projectID: UUID)
        case selectNextTabInWorktree(key: WorktreeKey)
        case selectPreviousTabInWorktree(key: WorktreeKey)
        case selectNextFlatTabInWorktree(key: WorktreeKey)
        case selectPreviousFlatTabInWorktree(key: WorktreeKey)
        case splitArea(SplitAreaRequest)
        case splitAreaInWorktree(key: WorktreeKey, request: SplitAreaRequest)
        case closeArea(projectID: UUID, areaID: UUID)
        case closeAreaInWorktree(key: WorktreeKey, areaID: UUID)
        case focusArea(projectID: UUID, areaID: UUID)
        case focusPaneLeft(projectID: UUID)
        case focusPaneRight(projectID: UUID)
        case focusPaneUp(projectID: UUID)
        case focusPaneDown(projectID: UUID)
        case movePaneLeft(projectID: UUID)
        case movePaneRight(projectID: UUID)
        case movePaneUp(projectID: UUID)
        case movePaneDown(projectID: UUID)
        case cycleNextTabAcrossPanes(projectID: UUID)
        case cyclePreviousTabAcrossPanes(projectID: UUID)
        case moveTab(projectID: UUID, request: TabMoveRequest)
        case moveTopLevelTab(projectID: UUID, request: TopLevelTabMoveRequest)
        case selectNextProject(projects: [Project], worktrees: [UUID: [Worktree]])
        case selectPreviousProject(projects: [Project], worktrees: [UUID: [Worktree]])
        case navigate(projectID: UUID, worktreeID: UUID, areaID: UUID, tabID: UUID?)
        case applyLayout(projectID: UUID, worktreePath: String, config: LayoutConfig)
    }

    private let selectionStore: any ActiveProjectSelectionStoring
    private let terminalViews: any TerminalViewRemoving
    private let workspacePersistence: any WorkspacePersisting
    var onProjectsEmptied: (([UUID]) -> Void)?
    var onProjectSelected: ((UUID) -> Void)?

    var activeProjectID: UUID?

    var activeWorktreeID: [UUID: UUID] = [:]

    private(set) var worktreeMRU: [WorktreeKey] = []

    struct PendingTabClose: Equatable {
        let key: WorktreeKey
        let areaID: UUID
        let tabID: UUID
    }

    struct PendingLayoutApply: Equatable {
        let projectID: UUID
        let worktreePath: String
        let layoutName: String
    }

    struct MaximizedPane: Equatable {
        let topLevelTabID: UUID
        let areaID: UUID
    }

    var workspaceRoots: [WorktreeKey: SplitNode] = [:]
    var focusedAreaID: [WorktreeKey: UUID] = [:]
    var topLevelTabOrder: [WorktreeKey: [UUID]] = [:]
    var topLevelTabLayouts: [WorktreeKey: TopLevelTabNode] = [:]
    var pendingLayoutApply: PendingLayoutApply?
    var maximizedPanes: [WorktreeKey: MaximizedPane] = [:]
    var pendingLastTabClose: PendingTabClose?
    var pendingProcessTabClose: PendingTabClose?
    let navigation = NavigationHistory()
    private var focusHistory: [WorktreeKey: [UUID]] = [:]

    init(
        selectionStore: any ActiveProjectSelectionStoring,
        terminalViews: any TerminalViewRemoving,
        workspacePersistence: any WorkspacePersisting
    ) {
        self.selectionStore = selectionStore
        self.terminalViews = terminalViews
        self.workspacePersistence = workspacePersistence
    }

    func restoreSelection(
        projects: [Project],
        worktrees: [UUID: [Worktree]],
        skippingProjectIDs: Set<UUID> = []
    ) {
        let snapshots: [WorkspaceSnapshot]
        do {
            snapshots = try workspacePersistence.loadWorkspaces()
        } catch {
            logger.error("Failed to load workspaces: \(error)")
            snapshots = []
        }
        let restorableSnapshots = snapshots.filter { !skippingProjectIDs.contains($0.projectID) }
        let restored = WorkspaceRestorer.restoreAll(
            from: restorableSnapshots,
            projects: projects,
            worktrees: worktrees
        )
        for entry in restored {
            workspaceRoots[entry.key] = entry.root
            focusedAreaID[entry.key] = entry.focusedAreaID
            topLevelTabOrder[entry.key] = entry.topLevelTabOrder
            topLevelTabLayouts[entry.key] = entry.topLevelTabLayout
        }

        let savedWorktreeIDs = selectionStore.loadActiveWorktreeIDs()
        for project in projects {
            let restoredKeysForProject = restored.map(\.key).filter { $0.projectID == project.id }
            guard !restoredKeysForProject.isEmpty else { continue }
            if let savedWorktreeID = savedWorktreeIDs[project.id],
               restoredKeysForProject.contains(where: { $0.worktreeID == savedWorktreeID })
            {
                activeWorktreeID[project.id] = savedWorktreeID
                continue
            }
            activeWorktreeID[project.id] = restoredKeysForProject[0].worktreeID
        }

        guard let id = selectionStore.loadActiveProjectID(),
              projects.contains(where: { $0.id == id }),
              activeWorktreeID[id] != nil
        else { return }
        activeProjectID = id
        recordCurrentNavigationEntry()
        recordActiveWorktreeUsage()
    }

    func saveWorkspaces() {
        let snapshots = WorkspaceRestorer.snapshotAll(
            workspaceRoots: workspaceRoots,
            focusedAreaID: focusedAreaID,
            topLevelTabOrder: topLevelTabOrder,
            topLevelTabLayouts: topLevelTabLayouts
        )
        do {
            try workspacePersistence.saveWorkspaces(snapshots)
        } catch {
            logger.error("Failed to save workspaces: \(error)")
        }
    }

    private func saveSelection() {
        selectionStore.saveActiveProjectID(activeProjectID)
        selectionStore.saveActiveWorktreeIDs(activeWorktreeID)
    }

    func activeWorktreeKey(for projectID: UUID) -> WorktreeKey? {
        guard let worktreeID = activeWorktreeID[projectID] else { return nil }
        return WorktreeKey(projectID: projectID, worktreeID: worktreeID)
    }

    private func recordActiveWorktreeUsage() {
        guard let projectID = activeProjectID,
              let key = activeWorktreeKey(for: projectID)
        else { return }
        worktreeMRU.removeAll { $0 == key }
        worktreeMRU.insert(key, at: 0)
    }

    func workspaceRoot(for projectID: UUID) -> SplitNode? {
        guard let key = activeWorktreeKey(for: projectID) else { return nil }
        return workspaceRoots[key]
    }

    func focusedAreaID(for projectID: UUID) -> UUID? {
        guard let key = activeWorktreeKey(for: projectID) else { return nil }
        return focusedAreaID[key]
    }

    func selectProject(_ project: Project, worktree: Worktree) {
        let wasActive = activeProjectID == project.id
        dispatch(.selectProject(
            projectID: project.id,
            worktreeID: worktree.id,
            worktreePath: worktree.path
        ))
        guard !wasActive else { return }
        onProjectSelected?(project.id)
    }

    func selectWorktree(projectID: UUID, worktree: Worktree) {
        dispatch(.selectWorktree(
            projectID: projectID,
            worktreeID: worktree.id,
            worktreePath: worktree.path
        ))
    }

    func openInitialTab(projectID: UUID, worktree: Worktree) {
        selectWorktree(projectID: projectID, worktree: worktree)
        guard !hasTabs(for: projectID) else { return }
        createTab(projectID: projectID)
    }

    func focusedArea(for projectID: UUID) -> TabArea? {
        guard let key = activeWorktreeKey(for: projectID),
              let root = workspaceRoots[key],
              let areaID = focusedAreaID[key]
        else { return nil }
        return root.findArea(id: areaID)
    }

    func allAreas(for projectID: UUID) -> [TabArea] {
        guard let key = activeWorktreeKey(for: projectID) else { return [] }
        return workspaceRoots[key]?.allAreas() ?? []
    }

    @discardableResult
    func ensureWorkspace(projectID: UUID, worktreeID: UUID, worktreePath: String) -> WorktreeKey {
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        guard workspaceRoots[key] == nil else { return key }
        let area = TabArea(projectPath: worktreePath)
        workspaceRoots[key] = .tabArea(area)
        focusedAreaID[key] = area.id
        topLevelTabOrder[key] = area.tabs.map(\.id)
        topLevelTabLayouts[key] = .group(TopLevelTabGroup(
            tabIDs: area.tabs.map(\.id),
            activeTabID: area.activeTabID
        ))
        saveWorkspaces()
        return key
    }

    func areas(for key: WorktreeKey) -> [TabArea] {
        workspaceRoots[key]?.allAreas() ?? []
    }

    func topLevelTabs(for key: WorktreeKey) -> [(area: TabArea, tab: TerminalTab)] {
        workspaceRoots[key]?.topLevelTabs(order: topLevelTabOrder[key] ?? []) ?? []
    }

    func topLevelTabShortcutIndices(for key: WorktreeKey) -> [UUID: Int] {
        Dictionary(uniqueKeysWithValues: topLevelTabs(for: key).enumerated().map {
            ($0.element.tab.id, $0.offset)
        })
    }

    func tabStripAreaID(for key: WorktreeKey, groupID: UUID) -> UUID? {
        guard topLevelTabLayouts[key]?.group(id: groupID) != nil else { return nil }
        return topLevelTabs(for: key, groupID: groupID).first?.area.id
            ?? workspaceRoots[key]?.allAreas().first?.id
    }

    func topLevelTabs(
        for key: WorktreeKey,
        groupID: UUID
    ) -> [(area: TabArea, tab: TerminalTab)] {
        guard let group = topLevelTabLayouts[key]?.group(id: groupID),
              let root = workspaceRoots[key]
        else { return [] }
        let tabsByID = Dictionary(uniqueKeysWithValues: root.topLevelTabs().map { ($0.tab.id, $0) })
        return group.tabIDs.compactMap { tabsByID[$0] }
    }

    func activeTopLevelTabID(for key: WorktreeKey) -> UUID? {
        guard let root = workspaceRoots[key],
              let areaID = focusedAreaID[key],
              let tab = root.findArea(id: areaID)?.activeTab
        else { return nil }
        return tab.parentTabID ?? tab.id
    }

    func visibleLayout(for key: WorktreeKey) -> VisiblePaneNode? {
        guard let root = workspaceRoots[key],
              let topLevelTabID = activeTopLevelTabID(for: key)
        else { return nil }
        return root.visibleLayout(forTopLevelTabID: topLevelTabID)
    }

    func visibleLayout(for key: WorktreeKey, groupID: UUID) -> VisiblePaneNode? {
        guard let root = workspaceRoots[key],
              let group = topLevelTabLayouts[key]?.group(id: groupID),
              let topLevelTabID = group.activeTabID
        else { return nil }
        return root.visibleLayout(forTopLevelTabID: topLevelTabID)
    }

    func reorderTopLevelTabs(
        for key: WorktreeKey,
        fromOffsets: IndexSet,
        toOffset: Int
    ) {
        var order = topLevelTabs(for: key).map(\.tab.id)
        guard let source = fromOffsets.first, source < order.count else { return }
        let tabs = Dictionary(uniqueKeysWithValues: topLevelTabs(for: key).map { ($0.tab.id, $0.tab) })
        let pinnedCount = order.prefix { tabs[$0]?.isPinned == true }.count
        let lowerBound = tabs[order[source]]?.isPinned == true ? 0 : pinnedCount
        let upperBound = tabs[order[source]]?.isPinned == true ? pinnedCount : order.count
        let destination = min(max(toOffset, lowerBound), upperBound)
        order.move(fromOffsets: fromOffsets, toOffset: destination)
        if let layout = topLevelTabLayouts[key] {
            for group in layout.allGroups() {
                group.tabIDs = order.filter { group.tabIDs.contains($0) }
            }
        }
        topLevelTabOrder[key] = order
        saveWorkspaces()
    }

    func reorderTopLevelTabs(
        for key: WorktreeKey,
        groupID: UUID,
        fromOffsets: IndexSet,
        toOffset: Int
    ) {
        guard let group = topLevelTabLayouts[key]?.group(id: groupID) else { return }
        var order = group.tabIDs
        guard let source = fromOffsets.first, source < order.count else { return }
        let tabs = Dictionary(uniqueKeysWithValues: topLevelTabs(for: key).map { ($0.tab.id, $0.tab) })
        let pinnedCount = order.prefix { tabs[$0]?.isPinned == true }.count
        let lowerBound = tabs[order[source]]?.isPinned == true ? 0 : pinnedCount
        let upperBound = tabs[order[source]]?.isPinned == true ? pinnedCount : order.count
        let destination = min(max(toOffset, lowerBound), upperBound)
        order.move(fromOffsets: fromOffsets, toOffset: destination)
        group.tabIDs = order
        var globalOrder = topLevelTabs(for: key).map(\.tab.id)
        let groupIDs = Set(order)
        var replacementIndex = 0
        for index in globalOrder.indices where groupIDs.contains(globalOrder[index]) {
            globalOrder[index] = order[replacementIndex]
            replacementIndex += 1
        }
        topLevelTabOrder[key] = globalOrder
        saveWorkspaces()
    }

    func togglePinTopLevelTab(_ tabID: UUID, for key: WorktreeKey) {
        guard let located = workspaceRoots[key]?.locateTab(id: tabID),
              located.tab.parentTabID == nil
        else { return }
        located.area.togglePin(tabID)
        guard let layout = topLevelTabLayouts[key] else { return }
        let tabsByID = Dictionary(uniqueKeysWithValues: topLevelTabs(for: key).map { ($0.tab.id, $0.tab) })
        let order = topLevelTabs(for: key).map(\.tab.id)
        let reordered = order.filter { tabsByID[$0]?.isPinned == true }
            + order.filter { tabsByID[$0]?.isPinned != true }
        for group in layout.allGroups() {
            group.tabIDs = reordered.filter { group.tabIDs.contains($0) }
        }
        topLevelTabOrder[key] = reordered
        saveWorkspaces()
    }

    func hasTabs(for projectID: UUID) -> Bool {
        allAreas(for: projectID).contains { !$0.tabs.isEmpty }
    }

    func hasTabs(for key: WorktreeKey) -> Bool {
        areas(for: key).contains { !$0.tabs.isEmpty }
    }

    func locatePane(paneID: UUID) -> (worktreeKey: WorktreeKey, pane: TerminalPaneState)? {
        for (key, root) in workspaceRoots {
            for area in root.allAreas() {
                for tab in area.tabs {
                    if let pane = tab.content.pane, pane.id == paneID {
                        return (key, pane)
                    }
                }
            }
        }
        return nil
    }

    struct PaneTabLocation {
        let worktreeKey: WorktreeKey
        let areaID: UUID
        let tab: TerminalTab
    }

    func locateTab(forPane paneID: UUID) -> PaneTabLocation? {
        for (key, root) in workspaceRoots {
            for area in root.allAreas() {
                for tab in area.tabs where tab.content.pane?.id == paneID {
                    return PaneTabLocation(worktreeKey: key, areaID: area.id, tab: tab)
                }
            }
        }
        return nil
    }

    func splitFocusedArea(direction: SplitDirection, projectID: UUID) {
        guard let area = focusedArea(for: projectID) else { return }
        dispatch(.splitArea(.init(
            projectID: projectID,
            areaID: area.id,
            direction: direction,
            position: .second
        )))
    }

    func toggleMaximize(
        areaID: UUID,
        topLevelTabID requestedTopLevelTabID: UUID? = nil,
        for projectID: UUID
    ) {
        guard let key = activeWorktreeKey(for: projectID),
              let root = workspaceRoots[key],
              let topLevelTabID = requestedTopLevelTabID ?? activeTopLevelTabID(for: key),
              let topLevelTab = root.locateTab(id: topLevelTabID),
              topLevelTab.tab.parentTabID == nil
        else { return }
        let maximizedPane = MaximizedPane(topLevelTabID: topLevelTabID, areaID: areaID)
        if maximizedPanes[key] == maximizedPane {
            maximizedPanes.removeValue(forKey: key)
            return
        }
        guard let visibleLayout = root.visibleLayout(forTopLevelTabID: topLevelTabID),
              visibleLayout.allPanes().count > 1,
              visibleLayout.allPanes().contains(where: { $0.area.id == areaID })
        else {
            maximizedPanes.removeValue(forKey: key)
            return
        }
        dispatch(.selectTab(
            projectID: projectID,
            areaID: topLevelTab.area.id,
            tabID: topLevelTabID
        ))
        dispatch(.focusArea(projectID: projectID, areaID: areaID))
        maximizedPanes[key] = maximizedPane
    }

    func closeArea(_ areaID: UUID, projectID: UUID) {
        guard let key = activeWorktreeKey(for: projectID),
              let area = workspaceRoots[key]?.findArea(id: areaID)
        else { return }
        guard let tabID = area.activeTabID else {
            dispatch(.closeAreaInWorktree(key: key, areaID: areaID))
            return
        }
        closeTab(tabID, areaID: areaID, key: key)
    }

    func createTab(projectID: UUID) {
        dispatch(.createTab(projectID: projectID, areaID: nil))
    }

    @discardableResult
    func openInBuiltInBrowser(_ url: URL?, profileID: UUID? = nil) -> Bool {
        guard BrowserPreferences.isEnabled,
              let projectID = activeProjectID
        else { return false }
        let areaID = focusedArea(for: projectID)?.id
        let resolvedProfileID = profileID ?? BrowserPreferences.defaultProfileID
        dispatch(.createBrowserTab(projectID: projectID, areaID: areaID, url: url, profileID: resolvedProfileID))
        return true
    }

    func createCommandTab(projectID: UUID, shortcut: CommandShortcut) {
        dispatch(.createCommandTab(
            CommandTabRequest(
                projectID: projectID,
                areaID: nil,
                name: shortcut.displayName,
                command: shortcut.trimmedCommand,
                closesOnCommandExit: false
            )
        ))
    }

    func createCommandTab(projectID: UUID, command: String) {
        dispatch(.createCommandTab(
            CommandTabRequest(
                projectID: projectID,
                areaID: nil,
                name: command,
                command: command,
                closesOnCommandExit: false
            )
        ))
    }

    func closeTab(_ tabID: UUID, projectID: UUID) {
        guard let key = activeWorktreeKey(for: projectID),
              let root = workspaceRoots[key],
              let areaID = focusedAreaID[key],
              let area = root.findArea(id: areaID)
        else { return }
        closeTab(tabID, areaID: area.id, key: key)
    }

    func closeTab(_ tabID: UUID, areaID: UUID, projectID: UUID) {
        guard let key = activeWorktreeKey(for: projectID) else { return }
        closeTab(tabID, areaID: areaID, key: key)
    }

    func closeTab(_ tabID: UUID, areaID: UUID, key: WorktreeKey) {
        guard let tab = workspaceRoots[key]?
            .findArea(id: areaID)?
            .tabs
            .first(where: { $0.id == tabID }),
            !tab.isPinned
        else { return }
        let surfaceKeys = lifecycleSurfaceKeys(tabID: tabID, areaID: areaID, key: key)
        guard !surfaceKeys.isEmpty else {
            proceedCloseAfterVeto(tabID, areaID: areaID, key: key)
            return
        }
        Task { @MainActor in
            let requests = surfaceKeys.map { surfaceKey in
                Task { @MainActor in
                    await ExtensionSurfaceBridgeRegistry.shared.requestBeforeClose(surfaceKey)
                }
            }
            var allowsClose = true
            for request in requests {
                let verdict = await request.value
                allowsClose = allowsClose && verdict == .allow
            }
            guard allowsClose else { return }
            proceedCloseAfterVeto(tabID, areaID: areaID, key: key)
        }
    }

    func closeTabs(_ tabIDs: [UUID], areaID: UUID, projectID: UUID) {
        for tabID in tabIDs {
            closeTab(tabID, areaID: areaID, projectID: projectID)
        }
    }

    private func proceedCloseAfterVeto(_ tabID: UUID, areaID: UUID, key: WorktreeKey) {
        if needsProcessConfirmation(tabID: tabID, areaID: areaID, key: key) {
            pendingProcessTabClose = PendingTabClose(key: key, areaID: areaID, tabID: tabID)
            return
        }
        closeTabWithLastCheck(tabID, areaID: areaID, key: key)
    }

    func forceCloseTab(_ tabID: UUID, areaID: UUID, projectID: UUID) {
        guard let key = activeWorktreeKey(for: projectID) else { return }
        forceCloseTab(tabID, areaID: areaID, key: key)
    }

    func forceCloseTab(_ tabID: UUID, areaID: UUID, key: WorktreeKey) {
        clearPendingProcessCloseIfMatching(tabID: tabID, areaID: areaID, key: key)
        unpinTabIfNeeded(tabID, areaID: areaID, key: key)
        dispatch(.closeTabInWorktree(key: key, areaID: areaID, tabID: tabID))
    }

    func forceCloseTab(instanceID: String) {
        for (key, root) in workspaceRoots {
            for area in root.allAreas() {
                for tab in area.tabs where tab.content.extensionState?.id.uuidString == instanceID {
                    forceCloseTab(tab.id, areaID: area.id, key: key)
                    return
                }
            }
        }
    }

    private func lifecycleSurfaceKeys(tabID: UUID, areaID: UUID, key: WorktreeKey) -> [LifecycleSurfaceKey] {
        guard let root = workspaceRoots[key],
              let area = root.findArea(id: areaID),
              let tab = area.tabs.first(where: { $0.id == tabID })
        else { return [] }
        let tabs = if tab.parentTabID == nil {
            root.allTabs().filter { $0.id == tab.id || $0.parentTabID == tab.id }
        } else {
            [tab]
        }
        return tabs.compactMap { relatedTab in
            guard let state = relatedTab.content.extensionState else { return nil }
            return LifecycleSurfaceKey(kind: .tab, instanceID: state.id.uuidString)
        }
    }

    func confirmCloseRunningTab() {
        guard let pending = pendingProcessTabClose else { return }
        pendingProcessTabClose = nil
        closeTabWithLastCheck(pending.tabID, areaID: pending.areaID, key: pending.key)
    }

    func cancelCloseRunningTab() {
        pendingProcessTabClose = nil
    }

    private func closeTabWithLastCheck(_ tabID: UUID, areaID: UUID, key: WorktreeKey) {
        if !ProjectLifecyclePreferences.keepOpenWhenNoTabs,
           isLastTabInWorktree(tabID, areaID: areaID, key: key)
        {
            pendingLastTabClose = PendingTabClose(key: key, areaID: areaID, tabID: tabID)
            return
        }
        dispatch(.closeTabInWorktree(key: key, areaID: areaID, tabID: tabID))
    }

    func confirmCloseLastTab() {
        guard let pending = pendingLastTabClose else { return }
        pendingLastTabClose = nil
        dispatch(.closeTabInWorktree(key: pending.key, areaID: pending.areaID, tabID: pending.tabID))
    }

    func cancelCloseLastTab() {
        pendingLastTabClose = nil
    }

    func allOpenTerminalTabItems(
        for projectID: UUID,
        projectName: String,
        worktreeLabel: (UUID) -> (name: String?, branch: String?)
    ) -> [OpenTerminalTabItem] {
        workspaceRoots
            .filter { $0.key.projectID == projectID }
            .flatMap { key, root in
                let label = worktreeLabel(key.worktreeID)
                return root.allAreas().flatMap { area in
                    area.tabs.compactMap { tab -> OpenTerminalTabItem? in
                        guard let pane = tab.content.pane else { return nil }
                        let command = TerminalCommandTracker.shared.lastSubmittedCommand(for: pane.id)
                            ?? pane.startupCommand
                        return OpenTerminalTabItem(
                            projectID: projectID,
                            worktreeID: key.worktreeID,
                            areaID: area.id,
                            tabID: tab.id,
                            title: tab.title,
                            workingDirectory: pane.currentWorkingDirectory ?? pane.projectPath,
                            command: command,
                            projectName: projectName,
                            worktreeName: label.name,
                            worktreeBranch: label.branch
                        )
                    }
                }
            }
    }

    func availableLayouts(for projectID: UUID) -> [LayoutDescriptor] {
        guard let path = activeWorktreePath(for: projectID) else { return [] }
        return LayoutConfig.discover(projectPath: path)
    }

    func requestApplyLayout(projectID: UUID, layoutName: String) {
        guard let path = activeWorktreePath(for: projectID) else { return }
        pendingLayoutApply = PendingLayoutApply(
            projectID: projectID,
            worktreePath: path,
            layoutName: layoutName
        )
    }

    func confirmApplyLayout() {
        guard let pending = pendingLayoutApply else { return }
        pendingLayoutApply = nil
        guard let config = LayoutConfig.load(projectPath: pending.worktreePath, name: pending.layoutName) else {
            logger.error("Failed to load layout '\(pending.layoutName)' at \(pending.worktreePath)")
            return
        }
        dispatch(.applyLayout(
            projectID: pending.projectID,
            worktreePath: pending.worktreePath,
            config: config
        ))
    }

    func cancelApplyLayout() {
        pendingLayoutApply = nil
    }

    private func activeWorktreePath(for projectID: UUID) -> String? {
        guard let key = activeWorktreeKey(for: projectID),
              let root = workspaceRoots[key]
        else { return nil }
        return root.allAreas().first?.projectPath
    }

    private func unpinTabIfNeeded(_ tabID: UUID, areaID: UUID, key: WorktreeKey) {
        guard let root = workspaceRoots[key],
              let area = root.findArea(id: areaID),
              let tab = area.tabs.first(where: { $0.id == tabID }),
              tab.isPinned
        else { return }
        area.togglePin(tabID)
    }

    private func isLastTabInWorktree(_ tabID: UUID, areaID: UUID, key: WorktreeKey) -> Bool {
        guard let root = workspaceRoots[key],
              let tab = root.findArea(id: areaID)?.tabs.first(where: { $0.id == tabID })
        else { return false }
        guard tab.parentTabID == nil else { return false }
        return root.allTabs().count(where: { $0.parentTabID == nil }) <= 1
    }

    private func needsProcessConfirmation(tabID: UUID, areaID: UUID, key: WorktreeKey) -> Bool {
        guard TabCloseConfirmationPreferences.confirmRunningProcess else { return false }
        guard let root = workspaceRoots[key],
              let area = root.findArea(id: areaID),
              let tab = area.tabs.first(where: { $0.id == tabID })
        else { return false }
        let tabs = if tab.parentTabID == nil {
            root.allTabs().filter { $0.id == tab.id || $0.parentTabID == tab.id }
        } else {
            [tab]
        }
        return tabs.compactMap { $0.content.pane?.id }
            .contains { terminalViews.needsConfirmQuit(for: $0) }
    }

    func selectTabByIndex(_ index: Int, projectID: UUID) {
        guard let key = activeWorktreeKey(for: projectID), index >= 0 else { return }
        let tabs = topLevelTabs(for: key)
        guard index < tabs.count, activeTopLevelTabID(for: key) != tabs[index].tab.id else { return }
        dispatch(.selectTabByIndex(projectID: projectID, index: index))
    }

    func selectNextTab(projectID: UUID) {
        dispatch(.selectNextTab(projectID: projectID))
    }

    func selectPreviousTab(projectID: UUID) {
        dispatch(.selectPreviousTab(projectID: projectID))
    }

    func activeTab(for projectID: UUID) -> TerminalTab? {
        focusedArea(for: projectID)?.activeTab
    }

    @discardableResult
    func inspectActiveBrowserElement() -> Bool {
        guard let projectID = activeProjectID,
              let tab = activeTab(for: projectID),
              let browserState = tab.content.browserState
        else { return false }
        if BrowserWebViewRegistry.shared.inspectElement(for: browserState.id) {
            browserState.pendingCommand = nil
        } else {
            browserState.pendingCommand = .inspectElement
        }
        return true
    }

    func togglePinActiveTab(projectID: UUID) {
        guard let key = activeWorktreeKey(for: projectID),
              let tabID = activeTopLevelTabID(for: key)
        else { return }
        togglePinTopLevelTab(tabID, for: key)
    }

    func dispatch(_ action: Action) {
        _ = dispatchReturningEffects(action)
    }

    @discardableResult
    func dispatchReturningEffects(_ action: Action) -> WorkspaceSideEffects {
        let extensionSnapshot = ExtensionEventEmitter.snapshot(from: self)
        defer {
            let after = ExtensionEventEmitter.snapshot(from: self)
            ExtensionEventEmitter.emit(before: extensionSnapshot, after: after)
        }

        switch action {
        case let .focusPaneLeft(projectID),
             let .focusPaneRight(projectID),
             let .focusPaneUp(projectID),
             let .focusPaneDown(projectID):
            if let key = activeWorktreeKey(for: projectID),
               maximizedPanes[key] != nil
            {
                clearActivePaneIndicators()
                return WorkspaceSideEffects()
            }
        default:
            break
        }

        if case let .focusArea(projectID, areaID) = action,
           let key = activeWorktreeKey(for: projectID),
           focusedAreaID[key] == areaID
        {
            clearActivePaneIndicators()
            return WorkspaceSideEffects()
        }

        if case let .selectTab(projectID, areaID, tabID) = action,
           let key = activeWorktreeKey(for: projectID),
           let root = workspaceRoots[key],
           let area = root.findArea(id: areaID),
           area.activeTabID == tabID,
           focusedAreaID[key] == areaID
        {
            clearActivePaneIndicators()
            return WorkspaceSideEffects()
        }

        let currentWorkspaceRootSignature = workspaceRootSignature(workspaceRoots)
        let currentTopLevelTabLayoutSignature = topLevelTabLayoutSignature(topLevelTabLayouts)
        var workspace = WorkspaceState(
            activeProjectID: activeProjectID,
            activeWorktreeID: activeWorktreeID,
            workspaceRoots: workspaceRoots,
            focusedAreaID: focusedAreaID,
            focusHistory: focusHistory,
            topLevelTabOrder: topLevelTabOrder,
            topLevelTabLayouts: topLevelTabLayouts,
            keepProjectOpenWhenEmpty: ProjectLifecyclePreferences.keepOpenWhenNoTabs
        )
        let effects = WorkspaceReducer.reduce(action: action, state: &workspace)
        if activeProjectID != workspace.activeProjectID {
            activeProjectID = workspace.activeProjectID
        }
        if activeWorktreeID != workspace.activeWorktreeID {
            activeWorktreeID = workspace.activeWorktreeID
        }
        if currentWorkspaceRootSignature != workspaceRootSignature(workspace.workspaceRoots) {
            workspaceRoots = workspace.workspaceRoots
        }
        if focusedAreaID != workspace.focusedAreaID {
            focusedAreaID = workspace.focusedAreaID
        }
        if focusHistory != workspace.focusHistory {
            focusHistory = workspace.focusHistory
        }
        if topLevelTabOrder != workspace.topLevelTabOrder {
            topLevelTabOrder = workspace.topLevelTabOrder
        }
        if currentTopLevelTabLayoutSignature != topLevelTabLayoutSignature(workspace.topLevelTabLayouts) {
            topLevelTabLayouts = workspace.topLevelTabLayouts
        }
        invalidateMaximizedAreas(for: action)
        reconcilePendingClosures()

        for paneID in effects.paneIDsToRemove {
            terminalViews.removeView(for: paneID)
            TerminalProgressStore.shared.resetPane(paneID)
            DetectedAgentStore.shared.resetPane(paneID)
        }

        if !effects.projectIDsToRemove.isEmpty {
            onProjectsEmptied?(effects.projectIDsToRemove)
        }

        for collapse in effects.deferredAreaCollapses {
            DispatchQueue.main.async { [weak self] in
                guard let self,
                      let root = self.workspaceRoots[collapse.key],
                      let area = root.findArea(id: collapse.areaID),
                      area.tabs.isEmpty
                else { return }
                self.dispatch(.closeAreaInWorktree(key: collapse.key, areaID: collapse.areaID))
            }
        }

        pruneNavigationHistory()
        recordCurrentNavigationEntry()
        recordActiveWorktreeUsage()

        clearActivePaneIndicators()

        saveWorkspaces()
        saveSelection()
        return effects
    }

    private func clearActivePaneIndicators() {
        if let activeTabID = NotificationNavigator.activeTabID(appState: self) {
            NotificationStore.shared.markAsRead(tabID: activeTabID)
        }

        if let activePaneID = NotificationNavigator.activePaneID(appState: self) {
            TerminalProgressStore.shared.clearCompletion(for: activePaneID)
            AgentStatusStore.shared.clearCompletion(for: activePaneID)
        }
    }

    func goBack() {
        step(delta: -1)
    }

    func goForward() {
        step(delta: 1)
    }

    private func step(delta: Int) {
        while true {
            let targetIndex = navigation.cursor + delta
            guard targetIndex >= 0, targetIndex < navigation.entries.count else { return }
            let target = navigation.entries[targetIndex]
            if applyNavigationEntry(target) {
                navigation.setCursor(targetIndex)
                return
            }
            navigation.removeEntry(at: targetIndex)
        }
    }

    private func applyNavigationEntry(_ entry: NavigationEntry) -> Bool {
        guard navigationEntryIsLive(entry) else { return false }
        navigation.performWithRecordingSuppressed {
            dispatch(.navigate(
                projectID: entry.projectID,
                worktreeID: entry.worktreeID,
                areaID: entry.areaID,
                tabID: entry.tabID
            ))
        }
        return true
    }

    private func currentNavigationEntry() -> NavigationEntry? {
        guard let projectID = activeProjectID,
              let worktreeID = activeWorktreeID[projectID]
        else { return nil }
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        guard let root = workspaceRoots[key],
              let areaID = focusedAreaID[key],
              let area = root.findArea(id: areaID)
        else { return nil }
        return NavigationEntry(
            projectID: projectID,
            worktreeID: worktreeID,
            areaID: areaID,
            tabID: area.activeTabID
        )
    }

    private func recordCurrentNavigationEntry() {
        guard let entry = currentNavigationEntry() else { return }
        navigation.record(entry)
    }

    private func pruneNavigationHistory() {
        let originalCount = navigation.entries.count
        navigation.removeEntries { !navigationEntryIsLive($0) }
        guard navigation.entries.count != originalCount else { return }
        guard let live = currentNavigationEntry(),
              let matchIndex = navigation.entries.lastIndex(of: live)
        else { return }
        navigation.setCursor(matchIndex)
    }

    private func navigationEntryIsLive(_ entry: NavigationEntry) -> Bool {
        let key = WorktreeKey(projectID: entry.projectID, worktreeID: entry.worktreeID)
        guard let root = workspaceRoots[key],
              let area = root.findArea(id: entry.areaID)
        else { return false }
        if let tabID = entry.tabID, !area.tabs.contains(where: { $0.id == tabID }) {
            return false
        }
        return true
    }

    private func workspaceRootSignature(_ roots: [WorktreeKey: SplitNode]) -> [WorktreeKey: UUID] {
        roots.mapValues(\.id)
    }

    private func topLevelTabLayoutSignature(
        _ layouts: [WorktreeKey: TopLevelTabNode]
    ) -> [WorktreeKey: UUID] {
        layouts.mapValues(\.id)
    }

    private func clearPendingProcessCloseIfMatching(tabID: UUID, areaID: UUID, key: WorktreeKey) {
        guard let pending = pendingProcessTabClose else { return }
        guard pending.key == key,
              pending.areaID == areaID,
              pending.tabID == tabID
        else { return }
        pendingProcessTabClose = nil
    }

    private func reconcilePendingClosures() {
        if let pending = pendingLastTabClose,
           !tabExists(tabID: pending.tabID, areaID: pending.areaID, key: pending.key)
        {
            pendingLastTabClose = nil
        }

        if let pending = pendingProcessTabClose,
           !tabExists(tabID: pending.tabID, areaID: pending.areaID, key: pending.key)
        {
            pendingProcessTabClose = nil
        }
    }

    private func tabExists(tabID: UUID, areaID: UUID, key: WorktreeKey) -> Bool {
        guard let root = workspaceRoots[key],
              let area = root.findArea(id: areaID)
        else { return false }
        return area.tabs.contains(where: { $0.id == tabID })
    }

    private func invalidateMaximizedAreas(for action: Action) {
        if case let .splitArea(req) = action,
           let key = activeWorktreeKey(for: req.projectID),
           maximizedPanes[key]?.areaID == req.areaID
        {
            maximizedPanes.removeValue(forKey: key)
        }

        if case let .removeWorktree(projectID, worktreeID, _, _) = action {
            maximizedPanes.removeValue(forKey: WorktreeKey(projectID: projectID, worktreeID: worktreeID))
        }

        for key in Array(maximizedPanes.keys) {
            guard let maximizedPane = maximizedPanes[key],
                  let root = workspaceRoots[key],
                  let visibleLayout = root.visibleLayout(forTopLevelTabID: maximizedPane.topLevelTabID),
                  visibleLayout.allPanes().count > 1,
                  visibleLayout.allPanes().contains(where: { $0.area.id == maximizedPane.areaID }),
                  activeTopLevelTabID(for: key) == maximizedPane.topLevelTabID
            else {
                maximizedPanes.removeValue(forKey: key)
                continue
            }
            if focusedAreaID[key] != maximizedPane.areaID {
                maximizedPanes.removeValue(forKey: key)
            }
        }
    }

    func focusArea(_ areaID: UUID, projectID: UUID) {
        dispatch(.focusArea(projectID: projectID, areaID: areaID))
    }

    func focusPaneLeft(projectID: UUID) {
        dispatch(.focusPaneLeft(projectID: projectID))
    }

    func focusPaneRight(projectID: UUID) {
        dispatch(.focusPaneRight(projectID: projectID))
    }

    func focusPaneUp(projectID: UUID) {
        dispatch(.focusPaneUp(projectID: projectID))
    }

    func focusPaneDown(projectID: UUID) {
        dispatch(.focusPaneDown(projectID: projectID))
    }

    func moveFocusedPaneLeft(projectID: UUID) {
        dispatch(.movePaneLeft(projectID: projectID))
    }

    func moveFocusedPaneRight(projectID: UUID) {
        dispatch(.movePaneRight(projectID: projectID))
    }

    func moveFocusedPaneUp(projectID: UUID) {
        dispatch(.movePaneUp(projectID: projectID))
    }

    func moveFocusedPaneDown(projectID: UUID) {
        dispatch(.movePaneDown(projectID: projectID))
    }

    func cycleNextTabAcrossPanes(projectID: UUID) {
        dispatch(.cycleNextTabAcrossPanes(projectID: projectID))
    }

    func cyclePreviousTabAcrossPanes(projectID: UUID) {
        dispatch(.cyclePreviousTabAcrossPanes(projectID: projectID))
    }

    func selectProjectByIndex(_ index: Int, projects: [Project], worktrees: [UUID: [Worktree]]) {
        guard index >= 0, index < projects.count else { return }
        let project = projects[index]
        let list = worktrees[project.id] ?? []
        guard let target = list.first(where: { $0.isPrimary }) ?? list.first else { return }
        selectProject(project, worktree: target)
    }

    func selectNextProject(projects: [Project], worktrees: [UUID: [Worktree]]) {
        dispatch(.selectNextProject(projects: projects, worktrees: worktrees))
    }

    func selectPreviousProject(projects: [Project], worktrees: [UUID: [Worktree]]) {
        dispatch(.selectPreviousProject(projects: projects, worktrees: worktrees))
    }

    func removeProject(_ projectID: UUID) {
        dispatch(.removeProject(projectID: projectID))
    }

    func removeWorktree(projectID: UUID, worktree: Worktree, replacement: Worktree?) {
        guard !worktree.isPrimary else { return }
        dispatch(.removeWorktree(
            projectID: projectID,
            worktreeID: worktree.id,
            replacementWorktreeID: replacement?.id,
            replacementWorktreePath: replacement?.path
        ))
    }
}
