import Foundation
import Testing

@testable import Muxy

@Suite("AppState.focusInternalPane")
@MainActor
struct AppStateFocusInternalPaneTests {
    private let testPath = "/tmp/test"

    @Test("focusInternalPane selects project, area, tab, and pane")
    func focusInternalPaneSelectsTarget() {
        let projectID = UUID()
        let worktreeID = UUID()
        let appState = makeAppState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = appState.workspaceRoots[key]!.allAreas().first!
        let tabID = area.activeTabID!

        appState.dispatch(.splitTabPane(
            projectID: projectID, areaID: area.id, tabID: tabID, direction: .horizontal
        ))

        let tab = area.tabs.first { $0.id == tabID }
        let secondPaneID = tab?.internalPanes?.allPanes().last?.id

        appState.focusInternalPane(
            projectID: projectID, areaID: area.id, tabID: tabID, paneID: secondPaneID!
        )

        #expect(appState.activeProjectID == projectID)
        #expect(appState.focusedAreaID(for: projectID) == area.id)
        #expect(area.activeTabID == tabID)
        #expect(tab?.focusedPaneID == secondPaneID)
    }

    @Test("focusInternalPane leaves workspace unchanged when target is missing")
    func focusInternalPaneNoOpForMissingTarget() {
        let projectID = UUID()
        let worktreeID = UUID()
        let appState = makeAppState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = appState.workspaceRoots[key]!.allAreas().first!
        let tabID = area.activeTabID!

        appState.focusInternalPane(
            projectID: UUID(), areaID: UUID(), tabID: UUID(), paneID: UUID()
        )

        #expect(appState.activeProjectID == projectID)
        #expect(appState.focusedAreaID(for: projectID) == area.id)
        #expect(area.activeTabID == tabID)
    }

    private func makeAppState(projectID: UUID, worktreeID: UUID) -> AppState {
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = TabArea(projectPath: testPath)
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = .tabArea(area)
        appState.focusedAreaID[key] = area.id
        return appState
    }
}

private final class WorkspacePersistenceStub: WorkspacePersisting {
    private var snapshots: [WorkspaceSnapshot] = []
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { snapshots }
    func saveWorkspaces(_ workspaces: [WorkspaceSnapshot]) throws { snapshots = workspaces }
}

@MainActor
private final class SelectionStoreStub: ActiveProjectSelectionStoring {
    private var activeProjectID: UUID?
    private var activeWorktreeIDs: [UUID: UUID] = [:]
    func loadActiveProjectID() -> UUID? { activeProjectID }
    func saveActiveProjectID(_ id: UUID?) { activeProjectID = id }
    func loadActiveWorktreeIDs() -> [UUID: UUID] { activeWorktreeIDs }
    func saveActiveWorktreeIDs(_ ids: [UUID: UUID]) { activeWorktreeIDs = ids }
}

@MainActor
private final class TerminalViewRemovingStub: TerminalViewRemoving {
    func removeView(for paneID: UUID) {}
    func needsConfirmQuit(for paneID: UUID) -> Bool { false }
}
