import Foundation
import Testing

@testable import Muxy

@Suite("Project auto-rename")
struct ProjectTests {
    @Test("new project is not name-customized")
    func newProjectNotCustomized() {
        let project = Project(name: "my-app", path: "/Users/dev/my-app")
        #expect(project.isNameCustomized == false)
    }

    @Test("decodes legacy project without isNameCustomized — name matches folder")
    func legacyDecodesMatchingName() throws {
        let json = """
        {
            "id": "\(UUID().uuidString)",
            "name": "my-app",
            "path": "/Users/dev/my-app",
            "sortOrder": 0,
            "createdAt": \(Date().timeIntervalSince1970)
        }
        """
        let project = try JSONDecoder().decode(Project.self, from: Data(json.utf8))
        #expect(project.isNameCustomized == false)
    }

    @Test("decodes legacy project without isNameCustomized — name differs from folder")
    func legacyDecodesDifferingName() throws {
        let json = """
        {
            "id": "\(UUID().uuidString)",
            "name": "My Custom Name",
            "path": "/Users/dev/my-app",
            "sortOrder": 0,
            "createdAt": \(Date().timeIntervalSince1970)
        }
        """
        let project = try JSONDecoder().decode(Project.self, from: Data(json.utf8))
        #expect(project.isNameCustomized == true)
    }

    @Test("decodes project with explicit isNameCustomized = true")
    func decodesExplicitCustomized() throws {
        let json = """
        {
            "id": "\(UUID().uuidString)",
            "name": "whatever",
            "path": "/Users/dev/whatever",
            "sortOrder": 0,
            "createdAt": \(Date().timeIntervalSince1970),
            "isNameCustomized": true
        }
        """
        let project = try JSONDecoder().decode(Project.self, from: Data(json.utf8))
        #expect(project.isNameCustomized == true)
    }
}

@Suite("ProjectStore auto-rename")
@MainActor
struct ProjectStoreAutoRenameTests {
    private func makeStore() -> (ProjectStore, InMemoryProjectPersistence) {
        let persistence = InMemoryProjectPersistence()
        let store = ProjectStore(persistence: persistence)
        return (store, persistence)
    }

    @Test("rename marks project as name-customized")
    func renameSetsFlag() {
        let (store, _) = makeStore()
        let project = Project(name: "alpha", path: "/foo/alpha")
        store.add(project)
        store.rename(id: project.id, to: "My Project")
        let updated = store.projects.first(where: { $0.id == project.id })
        #expect(updated?.name == "My Project")
        #expect(updated?.isNameCustomized == true)
    }

    @Test("updatePath auto-renames when not customized")
    func updatePathAutoRenames() {
        let (store, _) = makeStore()
        let project = Project(name: "alpha", path: "/foo/alpha")
        store.add(project)
        store.updatePath(id: project.id, to: "/foo/beta")
        let updated = store.projects.first(where: { $0.id == project.id })
        #expect(updated?.path.hasSuffix("beta") == true)
        #expect(updated?.name == "beta")
    }

    @Test("updatePath preserves name when customized")
    func updatePathPreservesCustomName() {
        let (store, _) = makeStore()
        let project = Project(name: "alpha", path: "/foo/alpha")
        store.add(project)
        store.rename(id: project.id, to: "My Project")
        store.updatePath(id: project.id, to: "/foo/beta")
        let updated = store.projects.first(where: { $0.id == project.id })
        #expect(updated?.path.hasSuffix("beta") == true)
        #expect(updated?.name == "My Project")
    }

    @Test("updatePath is no-op when path unchanged")
    func updatePathNoOp() {
        let (store, _) = makeStore()
        let project = Project(name: "alpha", path: "/foo/alpha")
        store.add(project)
        store.updatePath(id: project.id, to: "/foo/alpha")
        #expect(store.projects.count == 1)
        #expect(store.projects.first?.name == "alpha")
    }

}

@Suite("ProjectPathSyncService")
@MainActor
struct ProjectPathSyncServiceTests {
    @Test("single terminal CWD sync updates project, primary worktree, and workspace path")
    func syncSingleTerminal() throws {
        let context = makeContext(projectName: "alpha", projectPath: "/foo/alpha")
        ProjectPathSyncService.syncFromTerminalWorkingDirectory(
            projectID: context.project.id,
            worktreeID: context.worktree.id,
            path: "/foo/beta",
            appState: context.appState,
            projectStore: context.projectStore,
            worktreeStore: context.worktreeStore
        )

        let project = try #require(context.projectStore.projects.first)
        let worktree = try #require(context.worktreeStore.primary(for: project.id))
        let area = try #require(context.appState.workspaceRoot(for: project.id)?.allAreas().first)
        #expect(project.name == "beta")
        #expect(project.path.hasSuffix("beta"))
        #expect(worktree.name == "beta")
        #expect(worktree.path.hasSuffix("beta"))
        #expect(area.projectPath.hasSuffix("beta"))
        #expect(area.activeTab?.content.pane?.projectPath.hasSuffix("beta") == true)
    }

    @Test("multiple terminals skip CWD sync")
    func skipMultipleTerminals() throws {
        let context = makeContext(projectName: "alpha", projectPath: "/foo/alpha")
        context.appState.createTab(projectID: context.project.id)
        ProjectPathSyncService.syncFromTerminalWorkingDirectory(
            projectID: context.project.id,
            worktreeID: context.worktree.id,
            path: "/foo/beta",
            appState: context.appState,
            projectStore: context.projectStore,
            worktreeStore: context.worktreeStore
        )

        let project = try #require(context.projectStore.projects.first)
        let worktree = try #require(context.worktreeStore.primary(for: project.id))
        #expect(project.path == "/foo/alpha")
        #expect(project.name == "alpha")
        #expect(worktree.path == "/foo/alpha")
        #expect(worktree.name == "alpha")
    }

    @Test("manual project rename preserves name while syncing paths")
    func syncPreservesManualProjectName() throws {
        let context = makeContext(projectName: "alpha", projectPath: "/foo/alpha")
        context.projectStore.rename(id: context.project.id, to: "Custom")
        ProjectPathSyncService.syncFromTerminalWorkingDirectory(
            projectID: context.project.id,
            worktreeID: context.worktree.id,
            path: "/foo/beta",
            appState: context.appState,
            projectStore: context.projectStore,
            worktreeStore: context.worktreeStore
        )

        let project = try #require(context.projectStore.projects.first)
        let worktree = try #require(context.worktreeStore.primary(for: project.id))
        #expect(project.name == "Custom")
        #expect(project.path.hasSuffix("beta"))
        #expect(worktree.name == "beta")
        #expect(worktree.path.hasSuffix("beta"))
    }

    private func makeContext(projectName: String, projectPath: String) -> ProjectPathSyncContext {
        let projectPersistence = InMemoryProjectPersistence()
        let projectStore = ProjectStore(persistence: projectPersistence)
        let project = Project(name: projectName, path: projectPath)
        projectStore.add(project)
        let worktreeStore = WorktreeStore(
            persistence: InMemoryWorktreePersistence(),
            projects: projectStore.projects
        )
        worktreeStore.ensurePrimary(for: project)
        let appState = AppState(
            selectionStore: InMemorySelectionStore(),
            terminalViews: InMemoryTerminalViewRemover(),
            workspacePersistence: InMemoryWorkspacePersistence()
        )
        let worktree = worktreeStore.primary(for: project.id)!
        appState.selectProject(project, worktree: worktree)
        return ProjectPathSyncContext(
            project: project,
            worktree: worktree,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            appState: appState
        )
    }
}

private struct ProjectPathSyncContext {
    let project: Project
    let worktree: Worktree
    let projectStore: ProjectStore
    let worktreeStore: WorktreeStore
    let appState: AppState
}

private final class InMemoryProjectPersistence: ProjectPersisting, @unchecked Sendable {
    private var projects: [Project] = []

    func loadProjects() throws -> [Project] {
        projects
    }

    func saveProjects(_ projects: [Project]) throws {
        self.projects = projects
    }
}

private final class InMemoryWorktreePersistence: WorktreePersisting, @unchecked Sendable {
    private var worktrees: [UUID: [Worktree]] = [:]

    func loadWorktrees(projectID: UUID) throws -> [Worktree] {
        worktrees[projectID] ?? []
    }

    func saveWorktrees(_ worktrees: [Worktree], projectID: UUID) throws {
        self.worktrees[projectID] = worktrees
    }

    func removeWorktrees(projectID: UUID) throws {
        worktrees.removeValue(forKey: projectID)
    }
}

private final class InMemoryWorkspacePersistence: WorkspacePersisting, @unchecked Sendable {
    private var snapshots: [WorkspaceSnapshot] = []

    func loadWorkspaces() throws -> [WorkspaceSnapshot] {
        snapshots
    }

    func saveWorkspaces(_ workspaces: [WorkspaceSnapshot]) throws {
        snapshots = workspaces
    }
}

@MainActor
private final class InMemorySelectionStore: ActiveProjectSelectionStoring {
    private var activeProjectID: UUID?
    private var activeWorktreeIDs: [UUID: UUID] = [:]

    func loadActiveProjectID() -> UUID? {
        activeProjectID
    }

    func saveActiveProjectID(_ id: UUID?) {
        activeProjectID = id
    }

    func loadActiveWorktreeIDs() -> [UUID: UUID] {
        activeWorktreeIDs
    }

    func saveActiveWorktreeIDs(_ ids: [UUID: UUID]) {
        activeWorktreeIDs = ids
    }
}

@MainActor
private final class InMemoryTerminalViewRemover: TerminalViewRemoving {
    func removeView(for paneID: UUID) {}

    func needsConfirmQuit(for paneID: UUID) -> Bool {
        false
    }
}
