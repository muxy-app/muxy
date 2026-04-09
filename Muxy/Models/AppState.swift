import Foundation
import SwiftUI

@MainActor
@Observable
final class AppState {
    struct SplitAreaRequest {
        let projectID: UUID
        let areaID: UUID
        let direction: SplitDirection
        let position: SplitPosition
        let projectPath: String
    }

    enum Action {
        case selectProject(projectID: UUID, projectPath: String)
        case removeProject(projectID: UUID)
        case createTab(projectID: UUID, areaID: UUID?)
        case createVCSTab(projectID: UUID, areaID: UUID?)
        case closeTab(projectID: UUID, areaID: UUID, tabID: UUID)
        case selectTab(projectID: UUID, areaID: UUID, tabID: UUID)
        case selectTabByIndex(projectID: UUID, areaID: UUID?, index: Int)
        case selectNextTab(projectID: UUID)
        case selectPreviousTab(projectID: UUID)
        case splitArea(SplitAreaRequest)
        case closeArea(projectID: UUID, areaID: UUID)
        case focusArea(projectID: UUID, areaID: UUID)
        case focusPaneLeft(projectID: UUID)
        case focusPaneRight(projectID: UUID)
        case focusPaneUp(projectID: UUID)
        case focusPaneDown(projectID: UUID)
        case moveTab(projectID: UUID, request: TabMoveRequest)
        case selectNextProject(projects: [Project])
        case selectPreviousProject(projects: [Project])
    }

    private let selectionStore: any ActiveProjectSelectionStoring
    private let terminalViews: any TerminalViewRemoving
    private let workspacePersistence: any WorkspacePersisting
    var onProjectsEmptied: (([UUID]) -> Void)?

    var activeProjectID: UUID? {
        didSet { saveSelection() }
    }

    struct PendingTabClose: Equatable {
        let projectID: UUID
        let areaID: UUID
        let tabID: UUID
    }

    var sidebarVisible = true
    var workspaceRoots: [UUID: SplitNode] = [:]
    var focusedAreaID: [UUID: UUID] = [:]
    var pendingLastTabClose: PendingTabClose?
    var pendingProcessTabClose: PendingTabClose?
    private var focusHistory: [UUID: [UUID]] = [:]

    init(
        selectionStore: any ActiveProjectSelectionStoring,
        terminalViews: any TerminalViewRemoving,
        workspacePersistence: any WorkspacePersisting
    ) {
        self.selectionStore = selectionStore
        self.terminalViews = terminalViews
        self.workspacePersistence = workspacePersistence
    }

    func restoreSelection(projects: [Project]) {
        let restored = WorkspaceRestorer.restoreAll(
            from: workspacePersistence.loadWorkspaces(),
            validProjectIDs: Set(projects.map(\.id))
        )
        for entry in restored {
            workspaceRoots[entry.projectID] = entry.root
            focusedAreaID[entry.projectID] = entry.focusedAreaID
        }
        guard let id = selectionStore.loadActiveProjectID(),
              let project = projects.first(where: { $0.id == id })
        else { return }
        selectProject(project)
    }

    func saveWorkspaces() {
        let snapshots = WorkspaceRestorer.snapshotAll(
            workspaceRoots: workspaceRoots,
            focusedAreaID: focusedAreaID
        )
        workspacePersistence.saveWorkspaces(snapshots)
    }

    private func saveSelection() {
        selectionStore.saveActiveProjectID(activeProjectID)
    }

    func workspaceRoot(for projectID: UUID) -> SplitNode? {
        workspaceRoots[projectID]
    }

    func selectProject(_ project: Project) {
        dispatch(.selectProject(projectID: project.id, projectPath: project.path))
    }

    func focusedArea(for projectID: UUID) -> TabArea? {
        guard let root = workspaceRoots[projectID],
              let areaID = focusedAreaID[projectID]
        else { return nil }
        return root.findArea(id: areaID)
    }

    func allAreas(for projectID: UUID) -> [TabArea] {
        workspaceRoots[projectID]?.allAreas() ?? []
    }

    func splitFocusedArea(direction: SplitDirection, projectID: UUID) {
        guard let area = focusedArea(for: projectID) else { return }
        dispatch(.splitArea(.init(
            projectID: projectID,
            areaID: area.id,
            direction: direction,
            position: .second,
            projectPath: area.projectPath
        )))
    }

    func closeArea(_ areaID: UUID, projectID: UUID) {
        dispatch(.closeArea(projectID: projectID, areaID: areaID))
    }

    func createTab(projectID: UUID) {
        dispatch(.createTab(projectID: projectID, areaID: nil))
    }

    func createVCSTab(projectID: UUID) {
        dispatch(.createVCSTab(projectID: projectID, areaID: nil))
    }

    func closeTab(_ tabID: UUID, projectID: UUID) {
        guard let area = focusedArea(for: projectID) else { return }
        closeTab(tabID, areaID: area.id, projectID: projectID)
    }

    func closeTab(_ tabID: UUID, areaID: UUID, projectID: UUID) {
        if needsProcessConfirmation(tabID: tabID, areaID: areaID, projectID: projectID) {
            pendingProcessTabClose = PendingTabClose(projectID: projectID, areaID: areaID, tabID: tabID)
            return
        }
        closeTabWithLastCheck(tabID, areaID: areaID, projectID: projectID)
    }

    func forceCloseTab(_ tabID: UUID, areaID: UUID, projectID: UUID) {
        clearPendingProcessCloseIfMatching(tabID: tabID, areaID: areaID, projectID: projectID)
        unpinTabIfNeeded(tabID, areaID: areaID, projectID: projectID)
        dispatch(.closeTab(projectID: projectID, areaID: areaID, tabID: tabID))
    }

    func confirmCloseRunningTab() {
        guard let pending = pendingProcessTabClose else { return }
        pendingProcessTabClose = nil
        closeTabWithLastCheck(pending.tabID, areaID: pending.areaID, projectID: pending.projectID)
    }

    func cancelCloseRunningTab() {
        pendingProcessTabClose = nil
    }

    private func closeTabWithLastCheck(_ tabID: UUID, areaID: UUID, projectID: UUID) {
        if isLastTabInProject(tabID, areaID: areaID, projectID: projectID) {
            pendingLastTabClose = PendingTabClose(projectID: projectID, areaID: areaID, tabID: tabID)
            return
        }
        dispatch(.closeTab(projectID: projectID, areaID: areaID, tabID: tabID))
    }

    func confirmCloseLastTab() {
        guard let pending = pendingLastTabClose else { return }
        pendingLastTabClose = nil
        dispatch(.closeTab(projectID: pending.projectID, areaID: pending.areaID, tabID: pending.tabID))
    }

    func cancelCloseLastTab() {
        pendingLastTabClose = nil
    }

    private func unpinTabIfNeeded(_ tabID: UUID, areaID: UUID, projectID: UUID) {
        guard let root = workspaceRoots[projectID],
              let area = root.findArea(id: areaID),
              let tab = area.tabs.first(where: { $0.id == tabID }),
              tab.isPinned
        else { return }
        area.togglePin(tabID)
    }

    private func isLastTabInProject(_ tabID: UUID, areaID: UUID, projectID: UUID) -> Bool {
        guard let root = workspaceRoots[projectID] else { return false }
        let allAreas = root.allAreas()
        let totalTabs = allAreas.reduce(0) { $0 + $1.tabs.count }
        return totalTabs <= 1
    }

    private func needsProcessConfirmation(tabID: UUID, areaID: UUID, projectID: UUID) -> Bool {
        guard let root = workspaceRoots[projectID],
              let area = root.findArea(id: areaID),
              let tab = area.tabs.first(where: { $0.id == tabID }),
              let paneID = tab.content.pane?.id
        else { return false }
        return terminalViews.needsConfirmQuit(for: paneID)
    }

    func selectTabByIndex(_ index: Int, projectID: UUID) {
        dispatch(.selectTabByIndex(projectID: projectID, areaID: nil, index: index))
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

    func togglePinActiveTab(projectID: UUID) {
        guard let area = focusedArea(for: projectID),
              let tabID = area.activeTabID
        else { return }
        area.togglePin(tabID)
        saveWorkspaces()
    }

    func dispatch(_ action: Action) {
        var workspace = WorkspaceState(
            activeProjectID: activeProjectID,
            workspaceRoots: workspaceRoots,
            focusedAreaID: focusedAreaID,
            focusHistory: focusHistory
        )
        let effects = WorkspaceReducer.reduce(action: action, state: &workspace)
        activeProjectID = workspace.activeProjectID
        workspaceRoots = workspace.workspaceRoots
        focusedAreaID = workspace.focusedAreaID
        focusHistory = workspace.focusHistory
        reconcilePendingClosures()

        for paneID in effects.paneIDsToRemove {
            terminalViews.removeView(for: paneID)
        }

        if !effects.projectIDsToRemove.isEmpty {
            onProjectsEmptied?(effects.projectIDsToRemove)
        }

        saveWorkspaces()
    }

    private func clearPendingProcessCloseIfMatching(tabID: UUID, areaID: UUID, projectID: UUID) {
        guard let pending = pendingProcessTabClose else { return }
        guard pending.projectID == projectID,
              pending.areaID == areaID,
              pending.tabID == tabID
        else { return }
        pendingProcessTabClose = nil
    }

    private func reconcilePendingClosures() {
        if let pending = pendingLastTabClose,
           !tabExists(tabID: pending.tabID, areaID: pending.areaID, projectID: pending.projectID)
        {
            pendingLastTabClose = nil
        }

        if let pending = pendingProcessTabClose,
           !tabExists(tabID: pending.tabID, areaID: pending.areaID, projectID: pending.projectID)
        {
            pendingProcessTabClose = nil
        }
    }

    private func tabExists(tabID: UUID, areaID: UUID, projectID: UUID) -> Bool {
        guard let root = workspaceRoots[projectID],
              let area = root.findArea(id: areaID)
        else { return false }
        return area.tabs.contains(where: { $0.id == tabID })
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

    func selectProjectByIndex(_ index: Int, projects: [Project]) {
        guard index >= 0, index < projects.count else { return }
        selectProject(projects[index])
    }

    func selectNextProject(projects: [Project]) {
        dispatch(.selectNextProject(projects: projects))
    }

    func selectPreviousProject(projects: [Project]) {
        dispatch(.selectPreviousProject(projects: projects))
    }

    func removeProject(_ projectID: UUID) {
        dispatch(.removeProject(projectID: projectID))
    }
}
