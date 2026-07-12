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
        case closeTab(projectID: UUID, areaID: UUID, tabID: UUID)
        case closeTabInWorktree(key: WorktreeKey, areaID: UUID, tabID: UUID)
        case selectTab(projectID: UUID, areaID: UUID, tabID: UUID)
        case selectTabInWorktree(key: WorktreeKey, areaID: UUID, tabID: UUID)
        case selectTabByIndex(projectID: UUID, index: Int)
        case selectNextTab(projectID: UUID)
        case selectPreviousTab(projectID: UUID)
        case selectNextTabInWorktree(key: WorktreeKey)
        case selectPreviousTabInWorktree(key: WorktreeKey)
        case splitArea(SplitAreaRequest)
        case splitAreaInWorktree(key: WorktreeKey, request: SplitAreaRequest)
        case closeArea(projectID: UUID, areaID: UUID)
        case focusArea(projectID: UUID, areaID: UUID)
        case focusPaneLeft(projectID: UUID)
        case focusPaneRight(projectID: UUID)
        case focusPaneUp(projectID: UUID)
        case focusPaneDown(projectID: UUID)
        case cycleNextTabAcrossPanes(projectID: UUID)
        case cyclePreviousTabAcrossPanes(projectID: UUID)
        case moveTab(projectID: UUID, request: TabMoveRequest)
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

    var workspaceRoots: [WorktreeKey: SplitNode] = [:]
    var focusedAreaID: [WorktreeKey: UUID] = [:]
    var pendingLayoutApply: PendingLayoutApply?
    var maximizedAreaID: [WorktreeKey: UUID] = [:]
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
            focusedAreaID: focusedAreaID
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
        saveWorkspaces()
        return key
    }

    func areas(for key: WorktreeKey) -> [TabArea] {
        workspaceRoots[key]?.allAreas() ?? []
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

    func shortcutOffsets(for projectID: UUID) -> [UUID: Int] {
        guard let key = activeWorktreeKey(for: projectID) else { return [:] }
        if let maximizedAreaID = maximizedAreaID[key] {
            return [maximizedAreaID: 0]
        }
        var offsets: [UUID: Int] = [:]
        var running = 0
        for area in allAreas(for: projectID) {
            offsets[area.id] = running
            running += area.tabs.count
        }
        return offsets
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

    func toggleMaximize(areaID: UUID, for projectID: UUID) {
        guard let key = activeWorktreeKey(for: projectID),
              let root = workspaceRoots[key]
        else { return }
        guard case .split = root else {
            maximizedAreaID.removeValue(forKey: key)
            return
        }
        if maximizedAreaID[key] == areaID {
            maximizedAreaID.removeValue(forKey: key)
        } else {
            dispatch(.focusArea(projectID: projectID, areaID: areaID))
            maximizedAreaID[key] = areaID
        }
    }

    func closeArea(_ areaID: UUID, projectID: UUID) {
        dispatch(.closeArea(projectID: projectID, areaID: areaID))
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
        guard let surfaceKey = lifecycleSurfaceKey(tabID: tabID, areaID: areaID, key: key) else {
            proceedCloseAfterVeto(tabID, areaID: areaID, key: key)
            return
        }
        Task { @MainActor in
            let verdict = await ExtensionSurfaceBridgeRegistry.shared.requestBeforeClose(surfaceKey)
            guard verdict == .allow else { return }
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

    private func lifecycleSurfaceKey(tabID: UUID, areaID: UUID, key: WorktreeKey) -> LifecycleSurfaceKey? {
        guard let root = workspaceRoots[key],
              let area = root.findArea(id: areaID),
              let tab = area.tabs.first(where: { $0.id == tabID }),
              let state = tab.content.extensionState
        else { return nil }
        return LifecycleSurfaceKey(kind: .tab, instanceID: state.id.uuidString)
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
        guard let root = workspaceRoots[key] else { return false }
        let allAreas = root.allAreas()
        let totalTabs = allAreas.reduce(0) { $0 + $1.tabs.count }
        return totalTabs <= 1
    }

    private func needsProcessConfirmation(tabID: UUID, areaID: UUID, key: WorktreeKey) -> Bool {
        guard TabCloseConfirmationPreferences.confirmRunningProcess else { return false }
        guard let root = workspaceRoots[key],
              let area = root.findArea(id: areaID),
              let tab = area.tabs.first(where: { $0.id == tabID }),
              let paneID = tab.content.pane?.id
        else { return false }
        return terminalViews.needsConfirmQuit(for: paneID)
    }

    func selectTabByIndex(_ index: Int, projectID: UUID) {
        if let key = activeWorktreeKey(for: projectID),
           let areaID = maximizedAreaID[key],
           let root = workspaceRoots[key],
           let area = root.findArea(id: areaID)
        {
            guard index >= 0, index < area.tabs.count else { return }
            dispatch(.selectTab(projectID: projectID, areaID: areaID, tabID: area.tabs[index].id))
            return
        }
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
        guard let area = focusedArea(for: projectID),
              let tabID = area.activeTabID
        else { return }
        area.togglePin(tabID)
        saveWorkspaces()
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
               maximizedAreaID[key] != nil
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
        var workspace = WorkspaceState(
            activeProjectID: activeProjectID,
            activeWorktreeID: activeWorktreeID,
            workspaceRoots: workspaceRoots,
            focusedAreaID: focusedAreaID,
            focusHistory: focusHistory,
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
                self.dispatch(.closeArea(projectID: collapse.key.projectID, areaID: collapse.areaID))
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
           maximizedAreaID[key] == req.areaID
        {
            maximizedAreaID.removeValue(forKey: key)
        }

        if case let .removeWorktree(projectID, worktreeID, _, _) = action {
            maximizedAreaID.removeValue(forKey: WorktreeKey(projectID: projectID, worktreeID: worktreeID))
        }

        for key in Array(maximizedAreaID.keys) {
            guard let areaID = maximizedAreaID[key] else { continue }
            guard let root = workspaceRoots[key] else {
                maximizedAreaID.removeValue(forKey: key)
                continue
            }
            if case .tabArea = root {
                maximizedAreaID.removeValue(forKey: key)
                continue
            }
            if root.findArea(id: areaID) == nil {
                maximizedAreaID.removeValue(forKey: key)
                continue
            }
            if focusedAreaID[key] != areaID {
                maximizedAreaID.removeValue(forKey: key)
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
