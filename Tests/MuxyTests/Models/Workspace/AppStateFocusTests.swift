import Foundation
import Testing

@testable import Muxy

@Suite("AppState.focusInternalPane")
@MainActor
struct AppStateFocusInternalPaneTests {
    @Test("public method sets activeProjectID")
    func publicMethodSetsActiveProjectID() {
        let projectID = UUID()
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let areaID = UUID()
        let tabID = UUID()
        let paneID = UUID()

        appState.focusInternalPane(
            projectID: projectID,
            areaID: areaID,
            tabID: tabID,
            paneID: paneID
        )

        #expect(appState.activeProjectID == projectID)
    }

    @Test("dispatch with preconfigured workspace focuses correct area")
    func dispatchFocusesCorrectArea() {
        let projectID = UUID()
        let worktreeID = UUID()
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let firstArea = TabArea(projectPath: "/tmp/test")
        let secondArea = TabArea(projectPath: "/tmp/test")
        let tabID = secondArea.createTab()
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = .split(SplitBranch(
            direction: .horizontal,
            first: .tabArea(firstArea),
            second: .tabArea(secondArea)
        ))
        appState.focusedAreaID[key] = firstArea.id

        appState.dispatch(.focusInternalPane(
            projectID: projectID,
            areaID: secondArea.id,
            tabID: tabID,
            paneID: paneID
        ))

        #expect(appState.focusedAreaID[key] == secondArea.id)
    }
}

private let paneID = UUID()

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
