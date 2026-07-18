import Foundation
import Testing

@testable import Muxy

@Suite("MuxyAPI.Panes background materialization", .serialized)
@MainActor
struct MuxyAPIPanesMaterializeTests {
    private let testPath = "/tmp/test"

    @Test("readScreen materializes a pane in a background project")
    func readScreenMaterializesBackgroundProjectPane() async {
        let context = makeBackgroundPaneContext()
        defer { TerminalViewRegistry.shared.removeView(for: context.backgroundPaneID) }

        let result = await MuxyAPI.Panes.readScreen(
            paneIDString: context.backgroundPaneID.uuidString,
            lines: 10,
            appState: context.appState
        )

        guard case .success = result else {
            Issue.record("expected success, got \(result)")
            return
        }
        #expect(TerminalViewRegistry.shared.existingView(for: context.backgroundPaneID) != nil)
    }

    @Test("send materializes a pane in a background project")
    func sendMaterializesBackgroundProjectPane() async {
        let context = makeBackgroundPaneContext()
        defer { TerminalViewRegistry.shared.removeView(for: context.backgroundPaneID) }

        let result = await MuxyAPI.Panes.send(
            paneIDString: context.backgroundPaneID.uuidString,
            text: "ls",
            appState: context.appState
        )

        guard case .success = result else {
            Issue.record("expected success, got \(result)")
            return
        }
    }

    @Test("sendKeys materializes a pane in a background project")
    func sendKeysMaterializesBackgroundProjectPane() async {
        let context = makeBackgroundPaneContext()
        defer { TerminalViewRegistry.shared.removeView(for: context.backgroundPaneID) }

        let result = await MuxyAPI.Panes.sendKeys(
            paneIDString: context.backgroundPaneID.uuidString,
            key: "enter",
            appState: context.appState
        )

        guard case .success = result else {
            Issue.record("expected success, got \(result)")
            return
        }
    }

    @Test("readScreen returns paneNotFound for a pane absent from the workspace")
    func readScreenReturnsNotFoundForUnknownPane() async {
        let context = makeBackgroundPaneContext()

        let result = await MuxyAPI.Panes.readScreen(
            paneIDString: UUID().uuidString,
            lines: 10,
            appState: context.appState
        )

        guard case let .failure(error) = result, case .paneNotFound = error else {
            Issue.record("expected paneNotFound, got \(result)")
            return
        }
    }

    @Test("materialization reuses an already-registered view")
    func reusesRegisteredView() async {
        let context = makeBackgroundPaneContext()
        defer { TerminalViewRegistry.shared.removeView(for: context.backgroundPaneID) }

        let first = TerminalSurfaceMaterializer.materialize(
            paneID: context.backgroundPaneID,
            appState: context.appState
        )
        let second = TerminalSurfaceMaterializer.materialize(
            paneID: context.backgroundPaneID,
            appState: context.appState
        )

        #expect(first === second)
    }

    private struct Context {
        let appState: AppState
        let backgroundPaneID: UUID
    }

    private func makeBackgroundPaneContext() -> Context {
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let activeProjectID = UUID()
        let activeWorktreeID = UUID()
        addWorktree(to: appState, projectID: activeProjectID, worktreeID: activeWorktreeID)
        appState.activeProjectID = activeProjectID
        appState.activeWorktreeID[activeProjectID] = activeWorktreeID

        let backgroundKey = addWorktree(to: appState, projectID: UUID(), worktreeID: UUID())
        let backgroundPaneID = firstPaneID(in: appState, key: backgroundKey)
        return Context(appState: appState, backgroundPaneID: backgroundPaneID)
    }

    @discardableResult
    private func addWorktree(to appState: AppState, projectID: UUID, worktreeID: UUID) -> WorktreeKey {
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = TabArea(projectPath: testPath)
        appState.workspaceRoots[key] = .tabArea(area)
        appState.focusedAreaID[key] = area.id
        return key
    }

    private func firstPaneID(in appState: AppState, key: WorktreeKey) -> UUID {
        guard let root = appState.workspaceRoots[key],
              let pane = root.allAreas().first?.tabs.first?.content.pane
        else {
            Issue.record("no pane for worktree key \(key)")
            return UUID()
        }
        return pane.id
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
