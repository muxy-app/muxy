import Foundation
import Testing

@testable import Muxy

@Suite("AppState.worktreeMRU")
@MainActor
struct AppStateWorktreeMRUTests {
    @Test("selecting worktrees moves the most recent to the front")
    func recentSelectionMovesToFront() {
        let appState = makeAppState()
        let projectID = UUID()
        let a = UUID()
        let b = UUID()
        let c = UUID()

        appState.dispatch(.selectWorktree(projectID: projectID, worktreeID: a, worktreePath: "/tmp/a"))
        appState.dispatch(.selectWorktree(projectID: projectID, worktreeID: b, worktreePath: "/tmp/b"))
        appState.dispatch(.selectWorktree(projectID: projectID, worktreeID: c, worktreePath: "/tmp/c"))

        #expect(appState.worktreeMRU.map(\.worktreeID) == [c, b, a])
    }

    @Test("re-selecting an earlier worktree promotes it without duplicates")
    func reselectionPromotesWithoutDuplicates() {
        let appState = makeAppState()
        let projectID = UUID()
        let a = UUID()
        let b = UUID()
        let c = UUID()

        appState.dispatch(.selectWorktree(projectID: projectID, worktreeID: a, worktreePath: "/tmp/a"))
        appState.dispatch(.selectWorktree(projectID: projectID, worktreeID: b, worktreePath: "/tmp/b"))
        appState.dispatch(.selectWorktree(projectID: projectID, worktreeID: c, worktreePath: "/tmp/c"))
        appState.dispatch(.selectWorktree(projectID: projectID, worktreeID: a, worktreePath: "/tmp/a"))

        #expect(appState.worktreeMRU.map(\.worktreeID) == [a, c, b])
    }

    private func makeAppState() -> AppState {
        AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
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
