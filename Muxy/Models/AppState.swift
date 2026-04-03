import Foundation
import SwiftUI

@MainActor
@Observable
final class AppState {
    enum Action {
        case selectProject(projectID: UUID, projectPath: String)
        case removeProject(projectID: UUID)
        case createTab(projectID: UUID, areaID: UUID?)
        case closeTab(projectID: UUID, areaID: UUID, tabID: UUID)
        case selectTab(projectID: UUID, areaID: UUID, tabID: UUID)
        case selectTabByIndex(projectID: UUID, areaID: UUID?, index: Int)
        case selectNextTab(projectID: UUID)
        case selectPreviousTab(projectID: UUID)
        case splitArea(projectID: UUID, areaID: UUID, direction: SplitDirection, projectPath: String)
        case closeArea(projectID: UUID, areaID: UUID)
        case focusArea(projectID: UUID, areaID: UUID)
        case selectNextProject(projects: [Project])
        case selectPreviousProject(projects: [Project])
    }

    private let selectionStore: any ActiveProjectSelectionStoring
    private let terminalViews: any TerminalViewRemoving
    private let workspacePersistence: any WorkspacePersisting

    var activeProjectID: UUID? {
        didSet { saveSelection() }
    }

    var sidebarVisible = true
    var workspaceRoots: [UUID: SplitNode] = [:]
    var focusedAreaID: [UUID: UUID] = [:]
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
        dispatch(.splitArea(
            projectID: projectID,
            areaID: area.id,
            direction: direction,
            projectPath: area.projectPath
        ))
    }

    func closeArea(_ areaID: UUID, projectID: UUID) {
        dispatch(.closeArea(projectID: projectID, areaID: areaID))
    }

    func createTab(projectID: UUID) {
        dispatch(.createTab(projectID: projectID, areaID: nil))
    }

    func closeTab(_ tabID: UUID, projectID: UUID) {
        guard let area = focusedArea(for: projectID) else { return }
        dispatch(.closeTab(projectID: projectID, areaID: area.id, tabID: tabID))
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

        for paneID in effects.paneIDsToRemove {
            terminalViews.removeView(for: paneID)
        }

        saveWorkspaces()
    }

    func focusArea(_ areaID: UUID, projectID: UUID) {
        dispatch(.focusArea(projectID: projectID, areaID: areaID))
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
