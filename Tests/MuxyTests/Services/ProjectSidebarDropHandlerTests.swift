import Foundation
import Testing

@testable import Muxy

@Suite("ProjectSidebarDropHandler")
@MainActor
struct ProjectSidebarDropHandlerTests {
    @Test("directory drop adds and selects a project")
    func directoryDropAddsAndSelectsProject() async throws {
        let (appState, projectStore, worktreeStore, projectGroupStore) = makeStores()
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-sidebar-drop-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }

        let consumed = ProjectSidebarDropHandler.handle(
            providers: [FileURLItemProviderStub(item: directory as NSURL)]
        ) { path in
            ProjectOpenService.confirmProjectPathResult(
                path,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore,
                projectGroupStore: projectGroupStore
            )
        }

        await waitUntil { projectStore.storedProjects.count == 1 }

        #expect(consumed)
        #expect(projectStore.storedProjects.count == 1)
        #expect(projectStore.storedProjects.first?.path == directory.standardizedFileURL.path)
        #expect(appState.activeProjectID == projectStore.storedProjects.first?.id)
    }

    @Test("existing project drop does not duplicate")
    func existingProjectDropDoesNotDuplicate() async throws {
        let (appState, projectStore, worktreeStore, projectGroupStore) = makeStores()
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-sidebar-drop-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let project = Project(name: directory.lastPathComponent, path: directory.standardizedFileURL.path)
        projectStore.add(project)
        var handled = false

        let consumed = ProjectSidebarDropHandler.handle(
            providers: [FileURLItemProviderStub(item: directory as NSURL)]
        ) { path in
            handled = true
            return ProjectOpenService.confirmProjectPathResult(
                path,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore,
                projectGroupStore: projectGroupStore
            )
        }

        await waitUntil { handled }

        #expect(consumed)
        #expect(projectStore.storedProjects.count == 1)
        #expect(appState.activeProjectID == project.id)
    }

    @Test("non directory drop is ignored")
    func nonDirectoryDropIsIgnored() async throws {
        let (appState, projectStore, worktreeStore, projectGroupStore) = makeStores()
        let file = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-sidebar-drop-\(UUID().uuidString).txt")
        try Data().write(to: file)
        defer { try? FileManager.default.removeItem(at: file) }

        var handled = false
        let consumed = ProjectSidebarDropHandler.handle(
            providers: [FileURLItemProviderStub(item: file as NSURL)]
        ) { path in
            handled = true
            return ProjectOpenService.confirmProjectPathResult(
                path,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore,
                projectGroupStore: projectGroupStore
            )
        }

        await waitUntil { handled }

        #expect(consumed)
        #expect(projectStore.storedProjects.isEmpty)
        #expect(appState.activeProjectID == nil)
    }

    @Test("data file URL is parsed and passed through")
    func dataFileURLParsed() async {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-sidebar-drop-\(UUID().uuidString)")
        let provider = FileURLItemProviderStub(item: directory.dataRepresentation as NSData)
        var receivedPath: String?

        let consumed = ProjectSidebarDropHandler.handle(providers: [provider]) { path in
            receivedPath = path
            return .success
        }

        await waitUntil { receivedPath != nil }

        #expect(consumed)
        #expect(receivedPath == directory.path(percentEncoded: false))
    }

    @Test("drop processes every valid provider")
    func dropProcessesEveryValidProvider() async {
        let firstDirectory = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-sidebar-drop-\(UUID().uuidString)")
        let secondDirectory = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-sidebar-drop-\(UUID().uuidString)")
        let first = FileURLItemProviderStub(item: firstDirectory as NSURL)
        let second = FileURLItemProviderStub(item: secondDirectory as NSURL)
        var paths: [String] = []

        let consumed = ProjectSidebarDropHandler.handle(providers: [first, second]) { path in
            paths.append(path)
            return .success
        }

        await waitUntil { paths.count == 2 }

        #expect(consumed)
        #expect(paths == [
            firstDirectory.path(percentEncoded: false),
            secondDirectory.path(percentEncoded: false),
        ])
        #expect(first.loadCount == 1)
        #expect(second.loadCount == 1)
    }

    private func waitUntil(_ condition: () -> Bool, attempts: Int = 100) async {
        for _ in 0 ..< attempts {
            if condition() { return }
            await Task.yield()
        }
    }
}

@MainActor
private func makeStores() -> (AppState, ProjectStore, WorktreeStore, ProjectGroupStore) {
    let projectStore = ProjectStore(persistence: ProjectPersistenceStub())
    let worktreeStore = WorktreeStore(persistence: WorktreePersistenceStub(), projects: [])
    let appState = AppState(
        selectionStore: SelectionStoreStub(),
        terminalViews: TerminalViewRemovingStub(),
        workspacePersistence: WorkspacePersistenceStub()
    )
    let deviceStore = RemoteDeviceStore(persistence: InMemoryRemoteDevicePersistence())
    let projectGroupStore = ProjectGroupStore(
        persistence: ProjectGroupPersistenceStub(),
        remoteDeviceStore: deviceStore,
        workspaceContextSink: InMemoryWorkspaceContextSink()
    )
    return (appState, projectStore, worktreeStore, projectGroupStore)
}

private final class FileURLItemProviderStub: FileURLItemProviding, @unchecked Sendable {
    let item: NSSecureCoding?
    var loadCount = 0

    init(item: NSSecureCoding?) {
        self.item = item
    }

    func hasItemConformingToTypeIdentifier(_ typeIdentifier: String) -> Bool {
        typeIdentifier == "public.file-url"
    }

    func loadItem(
        forTypeIdentifier typeIdentifier: String,
        options: [AnyHashable: Any]?,
        completionHandler: (@Sendable (NSSecureCoding?, (any Error)?) -> Void)?
    ) {
        loadCount += 1
        completionHandler?(item, nil)
    }
}

private final class ProjectPersistenceStub: ProjectPersisting {
    private var projects: [Project] = []

    func loadProjects() throws -> [Project] {
        projects
    }

    func saveProjects(_ projects: [Project]) throws {
        self.projects = projects
    }
}

private final class WorktreePersistenceStub: WorktreePersisting {
    private var storage: [UUID: [Worktree]] = [:]

    func loadWorktrees(projectID: UUID) throws -> [Worktree] {
        storage[projectID] ?? []
    }

    func saveWorktrees(_ worktrees: [Worktree], projectID: UUID) throws {
        storage[projectID] = worktrees
    }

    func removeWorktrees(projectID: UUID) throws {
        storage.removeValue(forKey: projectID)
    }
}

private final class WorkspacePersistenceStub: WorkspacePersisting {
    private var snapshots: [WorkspaceSnapshot] = []

    func loadWorkspaces() throws -> [WorkspaceSnapshot] {
        snapshots
    }

    func saveWorkspaces(_ workspaces: [WorkspaceSnapshot]) throws {
        snapshots = workspaces
    }
}

@MainActor
private final class SelectionStoreStub: ActiveProjectSelectionStoring {
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
private final class TerminalViewRemovingStub: TerminalViewRemoving {
    func removeView(for paneID: UUID) {}

    func needsConfirmQuit(for paneID: UUID) -> Bool {
        false
    }
}
