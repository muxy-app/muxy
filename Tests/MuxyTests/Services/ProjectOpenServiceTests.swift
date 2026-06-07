import Foundation
import Testing

@testable import Muxy

@Suite("ProjectOpenService.confirmProjectPath")
@MainActor
struct ProjectOpenServiceTests {
    @Test("existing directory is added and selected")
    func existingDirectoryAddedAndSelected() throws {
        let (appState, projectStore, worktreeStore, projectGroupStore) = makeStores()
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-project-picker-test-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }

        let didConfirm = ProjectOpenService.confirmProjectPath(
            dir.path,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )

        #expect(didConfirm)
        #expect(projectStore.storedProjects.count == 1)
        #expect(appState.activeProjectID == projectStore.storedProjects.first?.id)
    }

    @Test("new project is added to selected group")
    func newProjectAddedToSelectedGroup() throws {
        let (appState, projectStore, worktreeStore, _) = makeStores()
        let group = ProjectGroup(name: "Work")
        let groupPersistence = ProjectGroupPersistenceStub(initial: [group])
        let projectGroupStore = ProjectGroupStore(persistence: groupPersistence)
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-project-picker-test-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }

        projectGroupStore.selectGroup(id: group.id)
        let didConfirm = ProjectOpenService.confirmProjectPath(
            dir.path,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )

        let addedProject = try #require(projectStore.storedProjects.first)
        #expect(didConfirm)
        #expect(projectStore.storedProjects.count == 1)
        #expect(groupPersistence.savedGroups?.first?.projectIDs == [addedProject.id])
    }

    @Test("new project remains visible in All Projects without group assignment")
    func newProjectPreservesAllProjectsBehavior() throws {
        let (appState, projectStore, worktreeStore, _) = makeStores()
        let group = ProjectGroup(name: "Work")
        let groupPersistence = ProjectGroupPersistenceStub(initial: [group])
        let projectGroupStore = ProjectGroupStore(persistence: groupPersistence)
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-project-picker-test-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }

        let didConfirm = ProjectOpenService.confirmProjectPath(
            dir.path,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )

        let addedProject = try #require(projectStore.storedProjects.first)
        #expect(didConfirm)
        #expect(projectGroupStore.filteredProjects(from: projectStore.projects).contains { $0.id == addedProject.id })
        #expect(groupPersistence.savedGroups == nil)
    }

    @Test("already-added path is selected without creating a duplicate project")
    func existingProjectSelectedWithoutDuplicate() throws {
        let (appState, projectStore, worktreeStore, projectGroupStore) = makeStores()
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-project-picker-test-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }

        #expect(ProjectOpenService.confirmProjectPath(
            dir.path,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        ))
        appState.activeProjectID = nil

        #expect(ProjectOpenService.confirmProjectPath(
            dir.path,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        ))
        #expect(projectStore.storedProjects.count == 1)
        #expect(appState.activeProjectID == projectStore.storedProjects.first?.id)
    }

    @Test("already-added path is added to selected group")
    func existingProjectAddedToSelectedGroup() throws {
        let (appState, projectStore, worktreeStore, _) = makeStores()
        let group = ProjectGroup(name: "Work")
        let groupPersistence = ProjectGroupPersistenceStub(initial: [group])
        let projectGroupStore = ProjectGroupStore(persistence: groupPersistence)
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-project-picker-test-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let project = Project(name: dir.lastPathComponent, path: dir.standardizedFileURL.path)
        projectStore.add(project)

        projectGroupStore.selectGroup(id: group.id)
        let didConfirm = ProjectOpenService.confirmProjectPath(
            dir.path,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )

        #expect(didConfirm)
        #expect(projectStore.storedProjects.count == 1)
        #expect(groupPersistence.savedGroups?.first?.projectIDs == [project.id])
    }

    @Test("already-added path recovers a missing primary worktree without creating a duplicate project")
    func existingProjectWithMissingPrimaryRecoversWithoutDuplicate() throws {
        let (appState, projectStore, worktreeStore, projectGroupStore) = makeStores()
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-project-picker-test-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let project = Project(name: dir.lastPathComponent, path: dir.standardizedFileURL.path)
        projectStore.add(project)

        let didConfirm = ProjectOpenService.confirmProjectPath(
            dir.path,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )

        #expect(didConfirm)
        #expect(projectStore.storedProjects.count == 1)
        #expect(worktreeStore.primary(for: project.id) != nil)
        #expect(appState.activeProjectID == project.id)
    }

    @Test("standardized equivalent path selects an existing project without creating a duplicate")
    func standardizedEquivalentPathDedupesExistingProject() throws {
        let (appState, projectStore, worktreeStore, projectGroupStore) = makeStores()
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-project-picker-test-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let project = Project(name: dir.lastPathComponent, path: dir.appendingPathComponent(".").path)
        projectStore.add(project)

        let result = ProjectOpenService.confirmProjectPathResult(
            dir.standardizedFileURL.path,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )

        #expect(result == .success)
        #expect(projectStore.storedProjects.count == 1)
        #expect(appState.activeProjectID == project.id)
    }

    @Test("regular file path is rejected")
    func regularFilePathRejected() throws {
        let (appState, projectStore, worktreeStore, projectGroupStore) = makeStores()
        let file = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-project-picker-test-\(UUID().uuidString)")
        try Data().write(to: file)
        defer { try? FileManager.default.removeItem(at: file) }

        let result = ProjectOpenService.confirmProjectPathResult(
            file.path,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore,
            createIfMissing: true
        )

        #expect(result == .notDirectory)
        #expect(!ProjectOpenService.confirmProjectPath(
            file.path,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore,
            createIfMissing: true
        ))
        #expect(projectStore.storedProjects.isEmpty)
        #expect(appState.activeProjectID == nil)
    }

    @Test("missing directory is rejected when creation is not requested")
    func missingDirectoryRejectedWithoutCreation() throws {
        let (appState, projectStore, worktreeStore, projectGroupStore) = makeStores()
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-project-picker-test-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: dir) }

        let didConfirm = ProjectOpenService.confirmProjectPath(
            dir.path,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )

        #expect(!didConfirm)
        #expect(!FileManager.default.fileExists(atPath: dir.path))
        #expect(projectStore.storedProjects.isEmpty)
        #expect(appState.activeProjectID == nil)
    }

    @Test("missing directory is created before adding when creation is confirmed")
    func missingDirectoryCreatedThenAdded() throws {
        let (appState, projectStore, worktreeStore, projectGroupStore) = makeStores()
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-project-picker-test-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: dir) }

        let didConfirm = ProjectOpenService.confirmProjectPath(
            dir.path,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore,
            createIfMissing: true
        )

        #expect(didConfirm)
        #expect(FileManager.default.fileExists(atPath: dir.path))
        #expect(projectStore.storedProjects.first?.path == dir.standardizedFileURL.path)
    }

    @Test("create failure returns create failed without adding a project")
    func createFailureReturnsCreateFailedWithoutAddingProject() {
        let (appState, projectStore, worktreeStore, projectGroupStore) = makeStores()
        let service = ProjectPathConfirmationService(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore,
            fileSystem: ProjectPathConfirmationFileSystemStub(
                state: .missing,
                createError: ProjectPathConfirmationFileSystemStub.Error()
            )
        )

        let result = service.confirm(path: "/tmp/muxy-create-failure", createIfMissing: true)

        #expect(result == .createFailed)
        #expect(projectStore.storedProjects.isEmpty)
        #expect(appState.activeProjectID == nil)
    }

    @Test("custom picker preference posts picker notification without opening Finder")
    func customPreferencePresentsProjectPickerWithoutOpeningFinder() throws {
        let (appState, projectStore, worktreeStore, projectGroupStore) = makeStores()
        let suiteName = "ProjectOpenServiceTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            Issue.record("Unable to create isolated UserDefaults suite")
            return
        }
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let preferences = ProjectPickerPreferences(defaults: defaults)
        let notificationCenter = NotificationCenter()
        let flag = NotificationFlag()
        let observer = notificationCenter.addObserver(
            forName: .openProjectPicker,
            object: nil,
            queue: nil
        ) { _ in
            flag.didPost = true
        }
        defer { notificationCenter.removeObserver(observer) }
        var didOpenFinder = false

        ProjectOpenService.openProjectViaPicker(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore,
            preferences: preferences,
            notificationCenter: notificationCenter,
            openWithFinder: { didOpenFinder = true }
        )

        #expect(flag.didPost)
        #expect(!didOpenFinder)
    }

    @Test("finder picker preference opens Finder without posting picker notification")
    func finderPreferencePresentsFinderWithoutProjectPickerNotification() throws {
        let (appState, projectStore, worktreeStore, projectGroupStore) = makeStores()
        let suiteName = "ProjectOpenServiceTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            Issue.record("Unable to create isolated UserDefaults suite")
            return
        }
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let preferences = ProjectPickerPreferences(defaults: defaults)
        preferences.mode = .finder
        let notificationCenter = NotificationCenter()
        let flag = NotificationFlag()
        let observer = notificationCenter.addObserver(
            forName: .openProjectPicker,
            object: nil,
            queue: nil
        ) { _ in
            flag.didPost = true
        }
        defer { notificationCenter.removeObserver(observer) }
        var didOpenFinder = false

        ProjectOpenService.openProjectViaPicker(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore,
            preferences: preferences,
            notificationCenter: notificationCenter,
            openWithFinder: { didOpenFinder = true }
        )

        #expect(!flag.didPost)
        #expect(didOpenFinder)
    }

    private func makeStores() -> (AppState, ProjectStore, WorktreeStore, ProjectGroupStore) {
        let projectStore = ProjectStore(persistence: ProjectPersistenceStub())
        let worktreeStore = WorktreeStore(persistence: WorktreePersistenceStub(), projects: [])
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let projectGroupStore = ProjectGroupStore(persistence: ProjectGroupPersistenceStub())
        return (appState, projectStore, worktreeStore, projectGroupStore)
    }
}

private final class NotificationFlag: @unchecked Sendable {
    var didPost = false
}

private struct ProjectPathConfirmationFileSystemStub: ProjectPathConfirmationFileSystem {
    struct Error: Swift.Error {}

    let state: ProjectPathConfirmationDirectoryState
    var createError: Swift.Error?

    func directoryState(atPath path: String) -> ProjectPathConfirmationDirectoryState {
        state
    }

    func createDirectory(atPath path: String) throws {
        if let createError {
            throw createError
        }
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
