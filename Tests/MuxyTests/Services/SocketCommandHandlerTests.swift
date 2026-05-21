import Foundation
import Testing

@testable import Muxy

@Suite("SocketCommandHandler")
@MainActor
struct SocketCommandHandlerTests {
    private let testPath = "/tmp/test"

    @Test("unknown command returns error")
    func unknownCommand() {
        let appState = makeAppState()
        let result = SocketCommandHandler.handle(cmd: "bogus", params: [:], appState: appState)
        #expect(result["ok"] as? Bool == false)
        #expect((result["error"] as? String)?.contains("Unknown command") == true)
    }

    @Test("split returns new pane ID")
    func splitReturnsNewPaneID() {
        let projectID = UUID()
        let worktreeID = UUID()
        let appState = makeAppState(projectID: projectID, worktreeID: worktreeID)

        let result = SocketCommandHandler.handle(
            cmd: "split",
            params: ["direction": "right"],
            appState: appState
        )

        #expect(result["ok"] as? Bool == true)
        let paneIDStr = result["paneID"] as? String
        #expect(paneIDStr != nil)
        #expect(UUID(uuidString: paneIDStr!) != nil)
    }

    @Test("split down returns new pane ID")
    func splitDownReturnsNewPaneID() {
        let projectID = UUID()
        let worktreeID = UUID()
        let appState = makeAppState(projectID: projectID, worktreeID: worktreeID)

        let result = SocketCommandHandler.handle(
            cmd: "split",
            params: ["direction": "down"],
            appState: appState
        )

        #expect(result["ok"] as? Bool == true)
        #expect(result["paneID"] as? String != nil)
    }

    @Test("split fails without active project")
    func splitFailsWithoutActiveProject() {
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )

        let result = SocketCommandHandler.handle(
            cmd: "split",
            params: ["direction": "right"],
            appState: appState
        )

        #expect(result["ok"] as? Bool == false)
        #expect((result["error"] as? String)?.contains("No active project") == true)
    }

    @Test("send fails with missing pane ID")
    func sendFailsMissingPaneID() {
        let appState = makeAppState()
        let result = SocketCommandHandler.handle(
            cmd: "send",
            params: ["text": "hello"],
            appState: appState
        )
        #expect(result["ok"] as? Bool == false)
        #expect((result["error"] as? String)?.contains("pane ID") == true)
    }

    @Test("send fails with invalid pane ID")
    func sendFailsInvalidPaneID() {
        let appState = makeAppState()
        let result = SocketCommandHandler.handle(
            cmd: "send",
            params: ["pane": "not-a-uuid", "text": "hello"],
            appState: appState
        )
        #expect(result["ok"] as? Bool == false)
    }

    @Test("send fails with nonexistent pane")
    func sendFailsNonexistentPane() {
        let appState = makeAppState()
        let result = SocketCommandHandler.handle(
            cmd: "send",
            params: ["pane": UUID().uuidString, "text": "hello"],
            appState: appState
        )
        #expect(result["ok"] as? Bool == false)
        #expect((result["error"] as? String)?.contains("Pane not found") == true)
    }

    @Test("send fails with missing text")
    func sendFailsMissingText() {
        let appState = makeAppState()
        let result = SocketCommandHandler.handle(
            cmd: "send",
            params: ["pane": UUID().uuidString],
            appState: appState
        )
        #expect(result["ok"] as? Bool == false)
        #expect((result["error"] as? String)?.contains("Missing text") == true)
    }

    @Test("send-keys fails with unsupported key")
    func sendKeysFailsUnsupportedKey() {
        let appState = makeAppState()
        let paneID = UUID()
        let result = SocketCommandHandler.handle(
            cmd: "send-keys",
            params: ["pane": paneID.uuidString, "key": "F13"],
            appState: appState
        )
        #expect(result["ok"] as? Bool == false)
    }

    @Test("send-keys fails with missing key")
    func sendKeysFailsMissingKey() {
        let appState = makeAppState()
        let result = SocketCommandHandler.handle(
            cmd: "send-keys",
            params: ["pane": UUID().uuidString],
            appState: appState
        )
        #expect(result["ok"] as? Bool == false)
        #expect((result["error"] as? String)?.contains("Missing key") == true)
    }

    @Test("read-screen fails with nonexistent pane")
    func readScreenFailsNonexistentPane() {
        let appState = makeAppState()
        let result = SocketCommandHandler.handle(
            cmd: "read-screen",
            params: ["pane": UUID().uuidString],
            appState: appState
        )
        #expect(result["ok"] as? Bool == false)
        #expect((result["error"] as? String)?.contains("Pane not found") == true)
    }

    @Test("close-pane fails with nonexistent pane")
    func closePaneFailsNonexistentPane() {
        let appState = makeAppState()
        let result = SocketCommandHandler.handle(
            cmd: "close-pane",
            params: ["pane": UUID().uuidString],
            appState: appState
        )
        #expect(result["ok"] as? Bool == false)
        #expect((result["error"] as? String)?.contains("Pane not found") == true)
    }

    @Test("close-pane fails with missing pane ID")
    func closePaneFailsMissingPaneID() {
        let appState = makeAppState()
        let result = SocketCommandHandler.handle(
            cmd: "close-pane",
            params: [:],
            appState: appState
        )
        #expect(result["ok"] as? Bool == false)
        #expect((result["error"] as? String)?.contains("pane ID") == true)
    }

    @Test("rename-pane fails with nonexistent pane")
    func renamePaneFailsNonexistentPane() {
        let appState = makeAppState()
        let result = SocketCommandHandler.handle(
            cmd: "rename-pane",
            params: ["pane": UUID().uuidString, "title": "Test"],
            appState: appState
        )
        #expect(result["ok"] as? Bool == false)
        #expect((result["error"] as? String)?.contains("Pane not found") == true)
    }

    @Test("rename-pane fails with missing title")
    func renamePaneFailsMissingTitle() {
        let appState = makeAppState()
        let result = SocketCommandHandler.handle(
            cmd: "rename-pane",
            params: ["pane": UUID().uuidString],
            appState: appState
        )
        #expect(result["ok"] as? Bool == false)
        #expect((result["error"] as? String)?.contains("Missing title") == true)
    }

    @Test("list-panes returns empty array when no panes")
    func listPanesEmpty() {
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )

        let result = SocketCommandHandler.handle(
            cmd: "list-panes",
            params: [:],
            appState: appState
        )

        #expect(result["ok"] as? Bool == true)
        let panes = result["panes"] as? [[String: Any]]
        #expect(panes != nil)
        #expect(panes?.isEmpty == true)
    }

    @Test("list-panes returns panes from workspace")
    func listPanesReturnsWorkspacePanes() {
        let projectID = UUID()
        let worktreeID = UUID()
        let appState = makeAppState(projectID: projectID, worktreeID: worktreeID)

        let result = SocketCommandHandler.handle(
            cmd: "list-panes",
            params: [:],
            appState: appState
        )

        #expect(result["ok"] as? Bool == true)
        let panes = result["panes"] as? [[String: Any]]
        #expect(panes != nil)
        #expect(panes!.count >= 1)

        let firstPane = panes!.first!
        #expect(firstPane["id"] as? String != nil)
        #expect(firstPane["projectID"] as? String == projectID.uuidString)
    }

    @Test("handleRequest parses valid JSON")
    func handleRequestParsesValidJSON() async {
        let appState = makeAppState()
        let json = "{\"cmd\":\"list-panes\"}"
        let data = Data(json.utf8)

        let response = await SocketCommandHandler.handleRequest(data, appState: appState)
        let parsed = try? JSONSerialization.jsonObject(with: response) as? [String: Any]

        #expect(parsed != nil)
        #expect(parsed?["ok"] as? Bool == true)
    }

    @Test("handleRequest returns error for invalid JSON")
    func handleRequestReturnsErrorForInvalidJSON() async {
        let appState = makeAppState()
        let data = Data("not json".utf8)

        let response = await SocketCommandHandler.handleRequest(data, appState: appState)
        let parsed = try? JSONSerialization.jsonObject(with: response) as? [String: Any]

        #expect(parsed != nil)
        #expect(parsed?["ok"] as? Bool == false)
        #expect((parsed?["error"] as? String)?.contains("Invalid JSON") == true)
    }

    @Test("handleRequest returns error for missing cmd")
    func handleRequestReturnsErrorForMissingCmd() async {
        let appState = makeAppState()
        let json = "{\"foo\":\"bar\"}"
        let data = Data(json.utf8)

        let response = await SocketCommandHandler.handleRequest(data, appState: appState)
        let parsed = try? JSONSerialization.jsonObject(with: response) as? [String: Any]

        #expect(parsed != nil)
        #expect(parsed?["ok"] as? Bool == false)
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
