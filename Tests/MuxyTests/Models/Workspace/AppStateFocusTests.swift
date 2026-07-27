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

    @Test("pane lookup finds an internal pane and its owning tab")
    func paneLookupFindsInternalPane() {
        let projectID = UUID()
        let worktreeID = UUID()
        let appState = makeAppState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = appState.workspaceRoots[key]!.allAreas().first!
        let tabID = area.activeTabID!
        appState.dispatch(.splitTabPane(
            projectID: projectID, areaID: area.id, tabID: tabID, direction: .horizontal
        ))
        let pane = area.tabs[0].internalPanes!.allPanes().last!

        #expect(appState.locatePane(paneID: pane.id)?.pane.id == pane.id)
        #expect(appState.locateTab(forPane: pane.id)?.tab.id == tabID)
        #expect(appState.locateTab(forPane: pane.id)?.pane.id == pane.id)
    }

    @Test("active pane lookup follows the focused internal pane")
    func activePaneLookupFollowsFocusedInternalPane() {
        let projectID = UUID()
        let worktreeID = UUID()
        let appState = makeAppState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = appState.workspaceRoots[key]!.allAreas().first!
        let tabID = area.activeTabID!
        appState.dispatch(.splitTabPane(
            projectID: projectID, areaID: area.id, tabID: tabID, direction: .horizontal
        ))
        let paneID = area.tabs[0].internalPanes!.allPanes().first!.id
        appState.focusInternalPane(projectID: projectID, areaID: area.id, tabID: tabID, paneID: paneID)

        #expect(NotificationNavigator.activePaneID(appState: appState) == paneID)
    }

    @Test("notification navigation focuses its exact internal pane")
    func notificationNavigationFocusesExactPane() {
        let projectID = UUID()
        let worktreeID = UUID()
        let appState = makeAppState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = appState.workspaceRoots[key]!.allAreas().first!
        let tabID = area.activeTabID!
        appState.dispatch(.splitTabPane(
            projectID: projectID, areaID: area.id, tabID: tabID, direction: .horizontal
        ))
        let paneID = area.tabs[0].internalPanes!.allPanes().first!.id
        let notification = MuxyNotification(
            paneID: paneID,
            projectID: projectID,
            worktreeID: worktreeID,
            areaID: area.id,
            tabID: tabID,
            worktreePath: testPath,
            source: .socket,
            title: "Title",
            body: "Body"
        )

        NotificationNavigator.navigate(
            to: notification,
            appState: appState,
            notificationStore: NotificationStore.shared
        )

        #expect(area.tabs[0].focusedPaneID == paneID)
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

    @Test("closeInternalPane preserves the surviving pane when closing the original")
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
        let survivingPaneID = tab!.internalPanes!.allPanes().last!.id

        appState.closeInternalPane(
            projectID: projectID, areaID: area.id, tabID: tabID, paneID: paneID!
        )

        let updatedTab = area.tabs.first { $0.id == tabID }
        #expect(updatedTab?.internalPanes?.allPanes().map(\.id) == [survivingPaneID])
        #expect(updatedTab?.focusedPaneID == survivingPaneID)
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

        #expect(appState.pendingProcessInternalPaneClose == .init(
            projectID: projectID,
            areaID: area.id,
            tabID: tabID,
            paneID: tab.focusedPaneID!
        ))
        #expect(appState.pendingProcessTabClose == nil)
        #expect(tab.internalPanes?.allPanes().count == 2)

        appState.confirmCloseRunningInternalPane()

        #expect(appState.pendingLastTabClose == nil)
        #expect(area.tabs.contains { $0.id == tabID })
        #expect(tab.internalPanes == nil)
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
