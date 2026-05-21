import Foundation
import Testing

@testable import Muxy

@Suite("AppState terminal session persistence")
@MainActor
struct AppStateTerminalSessionTests {
    @Test("closing terminal tab records closed tab and saves sessions once")
    func closingTerminalTabRecordsClosedTabAndSavesSessionsOnce() {
        let harness = makeHarness()
        let area = harness.area
        area.createTab()
        let tabID = area.tabs[0].id

        harness.appState.closeTab(tabID, areaID: area.id, projectID: harness.projectID)

        #expect(harness.terminalSessions.savedWorkspaceRoots.isEmpty)
        #expect(harness.terminalSessions.closedSnapshots.count == 1)
        #expect(harness.terminalSessions.closedWorkspaceRoots.count == 1)
        #expect(harness.terminalSessions.closedSnapshots.first?.projectID == harness.projectID)
        #expect(harness.terminalSessions.closedSnapshots.first?.projectPath == area.projectPath)
        #expect(!area.tabs.contains { $0.id == tabID })
    }

    @Test("force closing terminal tab uses the same single-save persistence path")
    func forceClosingTerminalTabUsesSamePersistencePath() {
        let harness = makeHarness()
        let area = harness.area
        area.createTab()
        let tabID = area.tabs[0].id

        harness.appState.forceCloseTab(tabID, areaID: area.id, projectID: harness.projectID)

        #expect(harness.terminalSessions.savedWorkspaceRoots.isEmpty)
        #expect(harness.terminalSessions.closedSnapshots.count == 1)
        #expect(harness.terminalSessions.closedWorkspaceRoots.count == 1)
        #expect(!area.tabs.contains { $0.id == tabID })
    }

    @Test("remote close terminal tab records closed tab through AppState persistence")
    func remoteCloseTerminalTabRecordsClosedTab() {
        let harness = makeHarness()
        let area = harness.area
        area.createTab()
        let tabID = area.tabs[0].id
        let delegate = RemoteServerDelegate(
            appState: harness.appState,
            projectStore: ProjectStore(persistence: ProjectPersistenceStub()),
            worktreeStore: WorktreeStore(persistence: WorktreePersistenceStub(), listGitWorktrees: { _ in [] })
        )

        delegate.closeTab(projectID: harness.projectID, areaID: area.id, tabID: tabID)

        #expect(harness.terminalSessions.savedWorkspaceRoots.isEmpty)
        #expect(harness.terminalSessions.closedSnapshots.count == 1)
        #expect(harness.terminalSessions.closedWorkspaceRoots.count == 1)
        #expect(!area.tabs.contains { $0.id == tabID })
    }

    @Test("ordinary terminal input tracking does not save terminal sessions")
    func ordinaryTerminalInputTrackingDoesNotSaveTerminalSessions() {
        let harness = makeHarness()
        let paneID = harness.area.tabs[0].content.pane!.id
        TerminalCommandTracker.shared.removePane(paneID)

        TerminalCommandTracker.shared.recordText("git status", paneID: paneID)
        TerminalCommandTracker.shared.recordBackspace(paneID: paneID)
        TerminalCommandTracker.shared.recordText("s", paneID: paneID)

        #expect(harness.terminalSessions.savedWorkspaceRoots.isEmpty)
        #expect(harness.terminalSessions.closedSnapshots.isEmpty)
        #expect(harness.terminalSessions.closedWorkspaceRoots.isEmpty)

        TerminalCommandTracker.shared.removePane(paneID)
    }

    private func makeHarness() -> Harness {
        let projectID = UUID()
        let worktreeID = UUID()
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = TabArea(projectPath: "/tmp/test")
        let terminalSessions = TerminalSessionStoreStub()
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub(),
            terminalSessions: terminalSessions
        )
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = .tabArea(area)
        appState.focusedAreaID[key] = area.id
        return Harness(
            appState: appState,
            terminalSessions: terminalSessions,
            projectID: projectID,
            area: area
        )
    }

    private struct Harness {
        let appState: AppState
        let terminalSessions: TerminalSessionStoreStub
        let projectID: UUID
        let area: TabArea
    }
}

@MainActor
private final class TerminalSessionStoreStub: TerminalSessionStoring {
    var sessionsByPaneID: [UUID: TerminalSessionSnapshot] = [:]
    var savedWorkspaceRoots: [[WorktreeKey: SplitNode]] = []
    var closedSnapshots: [ClosedTerminalTabSnapshot] = []
    var closedWorkspaceRoots: [[WorktreeKey: SplitNode]] = []

    func save(workspaceRoots: [WorktreeKey: SplitNode]) {
        savedWorkspaceRoots.append(workspaceRoots)
    }

    func recordClosedTerminalTab(_ snapshot: ClosedTerminalTabSnapshot, workspaceRoots: [WorktreeKey: SplitNode]) {
        closedSnapshots.append(snapshot)
        closedWorkspaceRoots.append(workspaceRoots)
    }

    func popLastClosedTerminalTab(projectID _: UUID, worktreeID _: UUID) -> ClosedTerminalTabSnapshot? {
        nil
    }

    func nextClosedSequence() -> Int64 {
        Int64(closedSnapshots.count + 1)
    }
}

private final class WorkspacePersistenceStub: WorkspacePersisting {
    private var snapshots: [WorkspaceSnapshot] = []
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { snapshots }
    func saveWorkspaces(_ workspaces: [WorkspaceSnapshot]) throws { snapshots = workspaces }
}

private final class ProjectPersistenceStub: ProjectPersisting {
    func loadProjects() throws -> [Project] { [] }
    func saveProjects(_: [Project]) throws {}
}

private final class WorktreePersistenceStub: WorktreePersisting {
    func loadWorktrees(projectID _: UUID) throws -> [Worktree] { [] }
    func saveWorktrees(_: [Worktree], projectID _: UUID) throws {}
    func removeWorktrees(projectID _: UUID) throws {}
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
