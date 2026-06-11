import Foundation
import Testing

@testable import Muxy

@Suite("AppState.restoreSelection")
@MainActor
struct AppStateRestoreTests {
    @Test("skipped project IDs do not restore their persisted tabs")
    func skippedProjectsAreNotRestored() {
        let project = Project(name: "api", path: "~/code/api")
        let worktree = Worktree(name: project.name, path: project.path, isPrimary: true)
        let snapshots = makeSnapshots(project: project, worktree: worktree)
        let appState = makeAppState(snapshots: snapshots)

        appState.restoreSelection(
            projects: [project],
            worktrees: [project.id: [worktree]],
            skippingProjectIDs: [project.id]
        )

        #expect(appState.workspaceRoot(for: project.id) == nil)
    }

    @Test("unskipped project IDs restore their persisted tabs")
    func unskippedProjectsAreRestored() {
        let project = Project(name: "api", path: "/tmp/api")
        let worktree = Worktree(name: project.name, path: project.path, isPrimary: true)
        let snapshots = makeSnapshots(project: project, worktree: worktree)
        let appState = makeAppState(snapshots: snapshots)

        appState.restoreSelection(
            projects: [project],
            worktrees: [project.id: [worktree]]
        )

        #expect(appState.workspaceRoot(for: project.id) != nil)
    }

    private func makeSnapshots(project: Project, worktree: Worktree) -> [WorkspaceSnapshot] {
        let key = WorktreeKey(projectID: project.id, worktreeID: worktree.id)
        let area = TabArea(projectPath: project.path)
        area.createTab()
        return WorkspaceRestorer.snapshotAll(
            workspaceRoots: [key: .tabArea(area)],
            focusedAreaID: [key: area.id]
        )
    }

    private func makeAppState(snapshots: [WorkspaceSnapshot]) -> AppState {
        AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub(snapshots: snapshots)
        )
    }
}

private final class WorkspacePersistenceStub: WorkspacePersisting {
    private var snapshots: [WorkspaceSnapshot]

    init(snapshots: [WorkspaceSnapshot] = []) {
        self.snapshots = snapshots
    }

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
