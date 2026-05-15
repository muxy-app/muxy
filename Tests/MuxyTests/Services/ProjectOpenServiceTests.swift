import Foundation
import Testing

@testable import Muxy

@Suite("ProjectOpenService.confirmProjectPath")
@MainActor
struct ProjectOpenServiceTests {
    @Test("existing directory is added and selected")
    func existingDirectoryAddedAndSelected() throws {
        let (appState, projectStore, worktreeStore) = makeStores()
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-project-picker-test-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }

        let didConfirm = ProjectOpenService.confirmProjectPath(
            dir.path,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore
        )

        #expect(didConfirm)
        #expect(projectStore.projects.count == 1)
        #expect(appState.activeProjectID == projectStore.projects.first?.id)
    }

    private func makeStores() -> (AppState, ProjectStore, WorktreeStore) {
        let projectStore = ProjectStore(persistence: ProjectPersistenceStub())
        let worktreeStore = WorktreeStore(persistence: WorktreePersistenceStub(), projects: [])
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        return (appState, projectStore, worktreeStore)
    }
}

private final class ProjectPersistenceStub: ProjectPersisting {
    private var projects: [Project] = []
    func loadProjects() throws -> [Project] { projects }
    func saveProjects(_ projects: [Project]) throws { self.projects = projects }
}

private final class WorktreePersistenceStub: WorktreePersisting {
    private var storage: [UUID: [Worktree]] = [:]
    func loadWorktrees(projectID: UUID) throws -> [Worktree] { storage[projectID] ?? [] }
    func saveWorktrees(_ worktrees: [Worktree], projectID: UUID) throws {
        storage[projectID] = worktrees
    }
    func removeWorktrees(projectID: UUID) throws { storage.removeValue(forKey: projectID) }
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
