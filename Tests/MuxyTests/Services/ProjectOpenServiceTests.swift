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

    @Test("already-added path is selected without creating a duplicate project")
    func existingProjectSelectedWithoutDuplicate() throws {
        let (appState, projectStore, worktreeStore) = makeStores()
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-project-picker-test-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }

        #expect(ProjectOpenService.confirmProjectPath(
            dir.path,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore
        ))
        appState.activeProjectID = nil

        #expect(ProjectOpenService.confirmProjectPath(
            dir.path,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore
        ))
        #expect(projectStore.projects.count == 1)
        #expect(appState.activeProjectID == projectStore.projects.first?.id)
    }

    @Test("already-added path recovers a missing primary worktree without creating a duplicate project")
    func existingProjectWithMissingPrimaryRecoversWithoutDuplicate() throws {
        let (appState, projectStore, worktreeStore) = makeStores()
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
            worktreeStore: worktreeStore
        )

        #expect(didConfirm)
        #expect(projectStore.projects.count == 1)
        #expect(worktreeStore.primary(for: project.id) != nil)
        #expect(appState.activeProjectID == project.id)
    }

    @Test("standardized equivalent path selects an existing project without creating a duplicate")
    func standardizedEquivalentPathDedupesExistingProject() throws {
        let (appState, projectStore, worktreeStore) = makeStores()
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
            worktreeStore: worktreeStore
        )

        #expect(result == .success)
        #expect(projectStore.projects.count == 1)
        #expect(appState.activeProjectID == project.id)
    }

    @Test("regular file path is rejected")
    func regularFilePathRejected() throws {
        let (appState, projectStore, worktreeStore) = makeStores()
        let file = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-project-picker-test-\(UUID().uuidString)")
        try Data().write(to: file)
        defer { try? FileManager.default.removeItem(at: file) }

        let result = ProjectOpenService.confirmProjectPathResult(
            file.path,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            createIfMissing: true
        )

        #expect(result == .notDirectory)
        #expect(!ProjectOpenService.confirmProjectPath(
            file.path,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            createIfMissing: true
        ))
        #expect(projectStore.projects.isEmpty)
        #expect(appState.activeProjectID == nil)
    }

    @Test("missing directory is rejected when creation is not requested")
    func missingDirectoryRejectedWithoutCreation() throws {
        let (appState, projectStore, worktreeStore) = makeStores()
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-project-picker-test-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: dir) }

        let didConfirm = ProjectOpenService.confirmProjectPath(
            dir.path,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore
        )

        #expect(!didConfirm)
        #expect(!FileManager.default.fileExists(atPath: dir.path))
        #expect(projectStore.projects.isEmpty)
        #expect(appState.activeProjectID == nil)
    }

    @Test("missing directory is created before adding when creation is confirmed")
    func missingDirectoryCreatedThenAdded() throws {
        let (appState, projectStore, worktreeStore) = makeStores()
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-project-picker-test-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: dir) }

        let didConfirm = ProjectOpenService.confirmProjectPath(
            dir.path,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            createIfMissing: true
        )

        #expect(didConfirm)
        #expect(FileManager.default.fileExists(atPath: dir.path))
        #expect(projectStore.projects.first?.path == dir.standardizedFileURL.path)
    }

    @Test("create failure returns create failed without adding a project")
    func createFailureReturnsCreateFailedWithoutAddingProject() {
        let (appState, projectStore, worktreeStore) = makeStores()
        let service = ProjectPathConfirmationService(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            fileSystem: ProjectPathConfirmationFileSystemStub(
                state: .missing,
                createError: ProjectPathConfirmationFileSystemStub.Error()
            )
        )

        let result = service.confirm(path: "/tmp/muxy-create-failure", createIfMissing: true)

        #expect(result == .createFailed)
        #expect(projectStore.projects.isEmpty)
        #expect(appState.activeProjectID == nil)
    }

    @Test("custom picker preference posts picker notification without opening Finder")
    func customPreferencePresentsProjectPickerWithoutOpeningFinder() throws {
        let (appState, projectStore, worktreeStore) = makeStores()
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
            preferences: preferences,
            notificationCenter: notificationCenter,
            openWithFinder: { didOpenFinder = true }
        )

        #expect(flag.didPost)
        #expect(!didOpenFinder)
    }

    @Test("finder picker preference opens Finder without posting picker notification")
    func finderPreferencePresentsFinderWithoutProjectPickerNotification() throws {
        let (appState, projectStore, worktreeStore) = makeStores()
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
            preferences: preferences,
            notificationCenter: notificationCenter,
            openWithFinder: { didOpenFinder = true }
        )

        #expect(!flag.didPost)
        #expect(didOpenFinder)
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
