import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionEventEmitter")
@MainActor
struct ExtensionEventEmitterTests {
    private let projectID = UUID()
    private let worktreeID = UUID()

    @Test("tab snapshot carries restore context")
    func tabSnapshotCarriesContext() {
        let appState = makeAppState()
        let area = firstArea(in: appState)
        area.createTab()

        let snapshot = ExtensionEventEmitter.snapshot(from: appState)
        let tabID = area.tabs.last!.id
        let context = snapshot.tabContext[tabID]

        #expect(context?.kind == "terminal")
        #expect(context?.projectID == projectID)
        #expect(context?.worktreeID == worktreeID)
        #expect(context?.areaID == area.id)
        #expect(context?.projectPath == "/tmp/project")
    }

    @Test("changing title or cwd marks the tab context dirty")
    func updatedTabContextDetectsChange() {
        let appState = makeAppState()
        let area = firstArea(in: appState)
        area.createTab()
        let pane = area.tabs.last!.content.pane!

        let before = ExtensionEventEmitter.snapshot(from: appState)
        pane.setWorkingDirectory("/tmp/project/sub")
        let after = ExtensionEventEmitter.snapshot(from: appState)

        let beforeContext = before.tabContext[area.tabs.last!.id]!
        let afterContext = after.tabContext[area.tabs.last!.id]!
        #expect(beforeContext.changesRelevantToRestore != afterContext.changesRelevantToRestore)
        #expect(afterContext.cwd == "/tmp/project/sub")
    }

    @Test("creating an adjacent tab focuses it in the snapshot diff")
    func adjacentTabFocusedInSnapshotDiff() {
        let appState = makeAppState()
        let area = firstArea(in: appState)
        let originalTabID = area.activeTabID!

        let before = ExtensionEventEmitter.snapshot(from: appState)
        appState.dispatch(.createTabAdjacent(
            projectID: projectID,
            areaID: area.id,
            tabID: originalTabID,
            side: .right
        ))
        let after = ExtensionEventEmitter.snapshot(from: appState)

        let newTabID = area.activeTabID!
        #expect(newTabID != originalTabID)
        #expect(before.activeTabIDPerArea[area.id] == originalTabID)
        #expect(after.activeTabIDPerArea[area.id] == newTabID)
        #expect(after.tabs.subtracting(before.tabs) == [newTabID])
    }

    @Test("closed tab context resolves from the before snapshot")
    func closedTabContextFromBeforeSnapshot() {
        let appState = makeAppState()
        let area = firstArea(in: appState)
        area.createTab()
        let tabID = area.tabs.last!.id

        let before = ExtensionEventEmitter.snapshot(from: appState)
        area.closeTab(tabID)
        let after = ExtensionEventEmitter.snapshot(from: appState)

        #expect(before.tabContext[tabID] != nil)
        #expect(after.tabContext[tabID] == nil)
        #expect(before.tabs.contains(tabID))
        #expect(!after.tabs.contains(tabID))
    }

    private func makeAppState() -> AppState {
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = TabArea(projectPath: "/tmp/project")
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = .tabArea(area)
        appState.focusedAreaID[key] = area.id
        return appState
    }

    private func firstArea(in appState: AppState) -> TabArea {
        appState.workspaceRoots.values.first!.allAreas().first!
    }
}

private final class WorkspacePersistenceStub: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
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
    func removeView(for _: UUID) {}
    func needsConfirmQuit(for _: UUID) -> Bool { false }
}
