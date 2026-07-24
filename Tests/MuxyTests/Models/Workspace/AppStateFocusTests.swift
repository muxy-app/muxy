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

@Suite("AppState.closeInternalPane")
@MainActor
struct AppStateCloseInternalPaneTests {
    private let testPath = "/tmp/test"

    @Test("closeInternalPane removes pane and clears splits when two panes remain")
    func closeInternalPaneRemovesPane() {
        let projectID = UUID()
        let worktreeID = UUID()
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

        let tabID = area.activeTabID!
        appState.dispatch(.splitTabPane(
            projectID: projectID, areaID: area.id, tabID: tabID, direction: .horizontal
        ))

        let tab = area.tabs.first { $0.id == tabID }
        let paneID = tab?.internalPanes?.allPanes().first?.id

        appState.closeInternalPane(
            projectID: projectID, areaID: area.id, tabID: tabID, paneID: paneID!
        )

        let updatedTab = area.tabs.first { $0.id == tabID }
        #expect(updatedTab?.internalPanes == nil)
        #expect(updatedTab?.focusedPaneID == nil)
    }

    @Test("closeInternalPane no-op when target missing")
    func closeInternalPaneNoOpForMissingTarget() {
        let projectID = UUID()
        let worktreeID = UUID()
        let appState = makeAppState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = appState.workspaceRoots[key]!.allAreas().first!
        let tabID = area.activeTabID!

        appState.closeInternalPane(
            projectID: UUID(), areaID: UUID(), tabID: UUID(), paneID: UUID()
        )

        #expect(appState.activeProjectID == projectID)
        #expect(appState.focusedAreaID(for: projectID) == area.id)
        #expect(area.activeTabID == tabID)
    }

    @Test("closeInternalPane requires confirmation before closing a running pane")
    func closeInternalPaneConfirmsRunningProcess() {
        let previousPreference = TabCloseConfirmationPreferences.confirmRunningProcess
        TabCloseConfirmationPreferences.confirmRunningProcess = true
        defer {
            TabCloseConfirmationPreferences.confirmRunningProcess = previousPreference
        }

        let projectID = UUID()
        let worktreeID = UUID()
        let terminalViews = TerminalViewRemovingStub()
        let appState = makeAppState(
            projectID: projectID,
            worktreeID: worktreeID,
            terminalViews: terminalViews
        )
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = appState.workspaceRoots[key]!.allAreas().first!
        let tabID = area.activeTabID!
        appState.dispatch(.splitTabPane(
            projectID: projectID,
            areaID: area.id,
            tabID: tabID,
            direction: .horizontal
        ))
        let tab = area.tabs.first { $0.id == tabID }!
        let paneID = tab.focusedPaneID!
        terminalViews.paneIDsRequiringConfirmation = [paneID]

        appState.closeInternalPane(
            projectID: projectID,
            areaID: area.id,
            tabID: tabID,
            paneID: paneID
        )

        #expect(tab.internalPanes?.allPanes().count == 2)
        #expect(appState.pendingProcessInternalPaneClose == .init(
            projectID: projectID,
            areaID: area.id,
            tabID: tabID,
            paneID: paneID
        ))

        appState.confirmCloseRunningInternalPane()

        #expect(appState.pendingProcessInternalPaneClose == nil)
        #expect(tab.internalPanes == nil)
        #expect(tab.focusedPaneID == nil)
    }

    @Test("closeTab checks the focused internal pane process")
    func closeTabChecksFocusedInternalPane() {
        let previousPreference = TabCloseConfirmationPreferences.confirmRunningProcess
        TabCloseConfirmationPreferences.confirmRunningProcess = true
        defer {
            TabCloseConfirmationPreferences.confirmRunningProcess = previousPreference
        }

        let projectID = UUID()
        let worktreeID = UUID()
        let terminalViews = TerminalViewRemovingStub()
        let appState = makeAppState(
            projectID: projectID,
            worktreeID: worktreeID,
            terminalViews: terminalViews
        )
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = appState.workspaceRoots[key]!.allAreas().first!
        let tabID = area.activeTabID!
        appState.dispatch(.splitTabPane(
            projectID: projectID,
            areaID: area.id,
            tabID: tabID,
            direction: .horizontal
        ))
        let tab = area.tabs.first { $0.id == tabID }!
        terminalViews.paneIDsRequiringConfirmation = [tab.focusedPaneID!]

        appState.closeTab(tabID, areaID: area.id, projectID: projectID)

        #expect(appState.pendingProcessTabClose == .init(key: key, areaID: area.id, tabID: tabID))
        #expect(tab.internalPanes?.allPanes().count == 2)
    }

    private func makeAppState(
        projectID: UUID,
        worktreeID: UUID,
        terminalViews: TerminalViewRemovingStub = TerminalViewRemovingStub()
    ) -> AppState {
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: terminalViews,
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
    var paneIDsRequiringConfirmation: Set<UUID> = []

    func removeView(for paneID: UUID) {}
    func needsConfirmQuit(for paneID: UUID) -> Bool {
        paneIDsRequiringConfirmation.contains(paneID)
    }
}
