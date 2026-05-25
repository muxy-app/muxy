import Foundation
import Testing

@testable import Muxy

@Suite("SocketCommandHandler")
@MainActor
struct SocketCommandHandlerTests {
    private let testPath = "/tmp/test"

    @Test("unknown command returns error")
    func unknownCommand() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("bogus", appState: appState)
        #expect(result.hasPrefix("error:"))
    }

    @Test("split-right returns new pane ID")
    func splitReturnsNewPaneID() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("split-right", appState: appState)
        #expect(!result.hasPrefix("error:"))
        #expect(UUID(uuidString: result) != nil)
    }

    @Test("split-down returns new pane ID")
    func splitDownReturnsNewPaneID() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("split-down", appState: appState)
        #expect(!result.hasPrefix("error:"))
        #expect(UUID(uuidString: result) != nil)
    }

    @Test("split-right with command returns new pane ID")
    func splitWithCommandReturnsNewPaneID() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("split-right||echo hello", appState: appState)
        #expect(!result.hasPrefix("error:"))
        #expect(UUID(uuidString: result) != nil)
    }

    @Test("split-right preserves commands containing pipes")
    func splitPreservesCommandPipes() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("split-right||echo a | wc", appState: appState)
        let paneID = UUID(uuidString: result)
        #expect(paneID != nil)
        #expect(paneID.flatMap { pane(with: $0, appState: appState)?.startupCommand } == "(echo a | wc); exec \"$0\" -l")
    }

    @Test("split fails without active project")
    func splitFailsWithoutActiveProject() async {
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let result = await SocketCommandHandler.handleRequest("split-right", appState: appState)
        #expect(result == "error:no active project")
    }

    @Test("send fails with missing args")
    func sendFailsMissingArgs() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("send|\(UUID().uuidString)", appState: appState)
        #expect(result.hasPrefix("error:"))
    }

    @Test("send-keys fails with unsupported key")
    func sendKeysFailsUnsupportedKey() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("send-keys|\(UUID().uuidString)|F13", appState: appState)
        #expect(result.hasPrefix("error:"))
    }

    @Test("send-keys fails with missing key")
    func sendKeysFailsMissingKey() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("send-keys|\(UUID().uuidString)", appState: appState)
        #expect(result.hasPrefix("error:"))
    }

    @Test("close-pane fails with nonexistent pane")
    func closePaneFailsNonexistentPane() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("close-pane|\(UUID().uuidString)", appState: appState)
        #expect(result.hasPrefix("error:pane not found"))
    }

    @Test("close-pane fails with invalid pane ID")
    func closePaneFailsInvalidPaneID() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("close-pane|not-a-uuid", appState: appState)
        #expect(result == "error:invalid pane ID")
    }

    @Test("rename-pane fails with nonexistent pane")
    func renamePaneFailsNonexistentPane() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("rename-pane|\(UUID().uuidString)|Test", appState: appState)
        #expect(result.hasPrefix("error:pane not found"))
    }

    @Test("rename-pane fails with missing title")
    func renamePaneFailsMissingTitle() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("rename-pane|\(UUID().uuidString)", appState: appState)
        #expect(result.hasPrefix("error:"))
    }

    @Test("list-panes returns empty when no panes")
    func listPanesEmpty() async {
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let result = await SocketCommandHandler.handleRequest("list-panes", appState: appState)
        #expect(result.isEmpty)
    }

    @Test("list-panes returns tab-separated pane info")
    func listPanesReturnsPanes() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("list-panes", appState: appState)
        #expect(!result.isEmpty)
        let fields = result.components(separatedBy: "\t")
        #expect(fields.count >= 4)
        #expect(UUID(uuidString: fields[0]) != nil)
    }

    @Test("split-right with from pane targets correct area")
    func splitWithFromPane() async {
        let appState = makeAppState()
        let firstPaneID = appState.workspaceRoots.values.first!.allAreas().first!.tabs.first!.content.pane!.id
        let result = await SocketCommandHandler.handleRequest("split-right|" + firstPaneID.uuidString + "|", appState: appState)
        #expect(!result.hasPrefix("error:"))
        #expect(UUID(uuidString: result) != nil)
    }

    private func pane(with paneID: UUID, appState: AppState) -> TerminalPaneState? {
        for root in appState.workspaceRoots.values {
            for area in root.allAreas() {
                for tab in area.tabs where tab.content.pane?.id == paneID {
                    return tab.content.pane
                }
            }
        }
        return nil
    }

    private func makeAppState(
        projectID: UUID = UUID(),
        worktreeID: UUID = UUID()
    ) -> AppState {
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
