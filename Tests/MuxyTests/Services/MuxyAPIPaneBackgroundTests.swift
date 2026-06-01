import Foundation
import Testing

@testable import Muxy

@Suite("MuxyAPI.Panes with background worktree")
@MainActor
struct MuxyAPIPaneBackgroundTests {
    @Test("send to a background worktree pane does not return paneNotFound")
    func sendToBackgroundWorktreePane() async {
        let projectID = UUID()
        let activeWorktreeID = UUID()
        let backgroundWorktreeID = UUID()
        let appState = makeAppState(
            projectID: projectID,
            activeWorktreeID: activeWorktreeID,
            backgroundWorktreeID: backgroundWorktreeID
        )
        let backgroundPaneID = paneID(in: appState, worktreeID: backgroundWorktreeID)

        let result = await MuxyAPI.Panes.send(
            paneIDString: backgroundPaneID.uuidString,
            text: "hello",
            appState: appState
        )

        guard case .success = result else {
            Issue.record("expected success for background pane, got \(result)")
            return
        }
    }

    @Test("sendKeys to a background worktree pane does not return paneNotFound")
    func sendKeysToBackgroundWorktreePane() async {
        let projectID = UUID()
        let activeWorktreeID = UUID()
        let backgroundWorktreeID = UUID()
        let appState = makeAppState(
            projectID: projectID,
            activeWorktreeID: activeWorktreeID,
            backgroundWorktreeID: backgroundWorktreeID
        )
        let backgroundPaneID = paneID(in: appState, worktreeID: backgroundWorktreeID)

        let result = await MuxyAPI.Panes.sendKeys(
            paneIDString: backgroundPaneID.uuidString,
            key: "enter",
            appState: appState
        )

        guard case .success = result else {
            Issue.record("expected success for background pane, got \(result)")
            return
        }
    }

    @Test("readScreen on a background worktree pane does not return paneNotFound")
    func readScreenOnBackgroundWorktreePane() async {
        let projectID = UUID()
        let activeWorktreeID = UUID()
        let backgroundWorktreeID = UUID()
        let appState = makeAppState(
            projectID: projectID,
            activeWorktreeID: activeWorktreeID,
            backgroundWorktreeID: backgroundWorktreeID
        )
        let backgroundPaneID = paneID(in: appState, worktreeID: backgroundWorktreeID)

        let result = await MuxyAPI.Panes.readScreen(
            paneIDString: backgroundPaneID.uuidString,
            lines: 10,
            appState: appState
        )

        guard case .success = result else {
            Issue.record("expected success for background pane, got \(result)")
            return
        }
    }

    @Test("send still returns paneNotFound for truly orphaned pane ids")
    func sendReturnsPaneNotFoundForOrphanedPane() async {
        let projectID = UUID()
        let activeWorktreeID = UUID()
        let appState = makeAppState(
            projectID: projectID,
            activeWorktreeID: activeWorktreeID,
            backgroundWorktreeID: nil
        )
        let orphanPaneID = UUID()

        let result = await MuxyAPI.Panes.send(
            paneIDString: orphanPaneID.uuidString,
            text: "hello",
            appState: appState
        )

        guard case let .failure(error) = result else {
            Issue.record("expected failure")
            return
        }
        guard case .paneNotFound = error else {
            Issue.record("expected paneNotFound, got \(error)")
            return
        }
    }

    @Test("active worktree pane is unaffected by the materializer path")
    func activeWorktreePaneUnaffected() async {
        let projectID = UUID()
        let activeWorktreeID = UUID()
        let appState = makeAppState(
            projectID: projectID,
            activeWorktreeID: activeWorktreeID,
            backgroundWorktreeID: nil
        )
        let activePaneID = paneID(in: appState, worktreeID: activeWorktreeID)

        let result = await MuxyAPI.Panes.send(
            paneIDString: activePaneID.uuidString,
            text: "hello",
            appState: appState
        )

        guard case .success = result else {
            Issue.record("expected success for active pane, got \(result)")
            return
        }
    }

    private func paneID(in appState: AppState, worktreeID: UUID) -> UUID {
        for (key, root) in appState.workspaceRoots where key.worktreeID == worktreeID {
            for area in root.allAreas() {
                for tab in area.tabs {
                    if let pane = tab.content.pane { return pane.id }
                }
            }
        }
        Issue.record("no pane found for worktree \(worktreeID)")
        return UUID()
    }

    private func makeAppState(
        projectID: UUID,
        activeWorktreeID: UUID,
        backgroundWorktreeID: UUID?
    ) -> AppState {
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let activeKey = WorktreeKey(projectID: projectID, worktreeID: activeWorktreeID)
        let activeArea = TabArea(projectPath: "/tmp/active")
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = activeWorktreeID
        appState.workspaceRoots[activeKey] = .tabArea(activeArea)
        appState.focusedAreaID[activeKey] = activeArea.id

        if let backgroundWorktreeID {
            let backgroundKey = WorktreeKey(projectID: projectID, worktreeID: backgroundWorktreeID)
            let backgroundArea = TabArea(projectPath: "/tmp/background")
            appState.workspaceRoots[backgroundKey] = .tabArea(backgroundArea)
            appState.focusedAreaID[backgroundKey] = backgroundArea.id
        }
        return appState
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
