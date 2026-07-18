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
        let projectGroupStore = ProjectGroupStore(persistence: groupPersistence, remoteDeviceStore: RemoteDeviceStore(persistence: InMemoryRemoteDevicePersistence()), workspaceContextSink: InMemoryWorkspaceContextSink())
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
        let projectGroupStore = ProjectGroupStore(persistence: groupPersistence, remoteDeviceStore: RemoteDeviceStore(persistence: InMemoryRemoteDevicePersistence()), workspaceContextSink: InMemoryWorkspaceContextSink())
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
        let projectGroupStore = ProjectGroupStore(persistence: groupPersistence, remoteDeviceStore: RemoteDeviceStore(persistence: InMemoryRemoteDevicePersistence()), workspaceContextSink: InMemoryWorkspaceContextSink())
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

    @Test("recently removed path restores project metadata and selects a fresh primary worktree")
    func recentlyRemovedPathRestoresProject() throws {
        let (appState, projectStore, worktreeStore, _) = makeStores()
        let group = ProjectGroup(name: "Work")
        let groupPersistence = ProjectGroupPersistenceStub(initial: [group])
        let projectGroupStore = ProjectGroupStore(
            persistence: groupPersistence,
            remoteDeviceStore: RemoteDeviceStore(persistence: InMemoryRemoteDevicePersistence()),
            workspaceContextSink: InMemoryWorkspaceContextSink()
        )
        projectGroupStore.selectGroup(id: group.id)
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-recent-project-test-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        var project = Project(name: "Custom Name", path: directory.path, sortOrder: 0)
        project.icon = "star.fill"
        project.logo = "stored-logo.png"
        project.iconColor = "purple"
        project.worktreesEnabled = true
        project.isPinned = true
        projectStore.add(project)
        projectStore.remove(id: project.id)
        worktreeStore.removeProject(project.id)

        let result = ProjectOpenService.confirmProjectPathResult(
            directory.path,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )

        let restored = try #require(projectStore.storedProjects.first)
        #expect(result == .success)
        #expect(restored.id == project.id)
        #expect(restored.name == "Custom Name")
        #expect(restored.icon == "star.fill")
        #expect(restored.logo == "stored-logo.png")
        #expect(restored.iconColor == "purple")
        #expect(restored.worktreesEnabled)
        #expect(restored.isPinned)
        #expect(projectStore.recentlyRemovedProjects.isEmpty)
        #expect(worktreeStore.primary(for: project.id) != nil)
        #expect(appState.activeProjectID == project.id)
        #expect(groupPersistence.savedGroups?.first?.projectIDs == [project.id])
    }

    @Test("missing recently removed path stays archived")
    func missingRecentlyRemovedPathStaysArchived() {
        let (appState, projectStore, worktreeStore, projectGroupStore) = makeStores()
        let project = Project(name: "Missing", path: "/tmp/missing-recent-project")
        projectStore.add(project)
        projectStore.remove(id: project.id)

        #expect(throws: ProjectOpenService.RestoreError.missingDirectory(project.path)) {
            try ProjectOpenService.restoreRecentlyRemovedProject(
                id: project.id,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore,
                projectGroupStore: projectGroupStore,
                fileSystem: ProjectPathConfirmationFileSystemStub(state: .missing)
            )
        }
        #expect(projectStore.storedProjects.isEmpty)
        #expect(projectStore.recentlyRemovedProjects.first?.id == project.id)
        #expect(worktreeStore.primary(for: project.id) == nil)
        #expect(appState.activeProjectID == nil)
    }

    @Test("failed recent restore stays archived and is not selected")
    func failedRecentlyRemovedRestoreStaysArchived() {
        let persistence = ProjectPersistenceStub()
        let projectStore = ProjectStore(persistence: persistence)
        let worktreeStore = WorktreeStore(persistence: WorktreePersistenceStub(), projects: [])
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let group = ProjectGroup(name: "Work")
        let groupPersistence = ProjectGroupPersistenceStub(initial: [group])
        let projectGroupStore = ProjectGroupStore(
            persistence: groupPersistence,
            remoteDeviceStore: RemoteDeviceStore(persistence: InMemoryRemoteDevicePersistence()),
            workspaceContextSink: InMemoryWorkspaceContextSink()
        )
        projectGroupStore.selectGroup(id: group.id)
        let project = Project(name: "Repo", path: "/tmp/repo")
        projectStore.add(project)
        projectStore.remove(id: project.id)
        persistence.projectSaveError = ProjectPersistenceStub.SaveError()

        #expect(throws: ProjectOpenService.RestoreError.persistenceFailed) {
            try ProjectOpenService.restoreRecentlyRemovedProject(
                id: project.id,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore,
                projectGroupStore: projectGroupStore,
                fileSystem: ProjectPathConfirmationFileSystemStub(state: .directory)
            )
        }
        #expect(projectStore.storedProjects.isEmpty)
        #expect(projectStore.recentlyRemovedProjects.first?.id == project.id)
        #expect(worktreeStore.primary(for: project.id) == nil)
        #expect(appState.activeProjectID == nil)
        #expect(groupPersistence.savedGroups == nil)
    }

    @Test("failed recent restore preserves pre-existing worktrees")
    func failedRecentlyRemovedRestorePreservesExistingWorktrees() {
        let persistence = ProjectPersistenceStub()
        let projectStore = ProjectStore(persistence: persistence)
        let worktreeStore = WorktreeStore(persistence: WorktreePersistenceStub(), projects: [])
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let projectGroupStore = ProjectGroupStore(
            persistence: ProjectGroupPersistenceStub(),
            remoteDeviceStore: RemoteDeviceStore(persistence: InMemoryRemoteDevicePersistence()),
            workspaceContextSink: InMemoryWorkspaceContextSink()
        )
        let project = Project(name: "Repo", path: "/tmp/repo")
        projectStore.add(project)
        projectStore.remove(id: project.id)
        let secondary = Worktree(
            name: "Feature",
            path: "/tmp/repo-feature",
            branch: "feature",
            isPrimary: false
        )
        worktreeStore.add(secondary, to: project.id)
        let existingWorktrees = worktreeStore.list(for: project.id)
        persistence.projectSaveError = ProjectPersistenceStub.SaveError()

        #expect(throws: ProjectOpenService.RestoreError.persistenceFailed) {
            try ProjectOpenService.restoreRecentlyRemovedProject(
                id: project.id,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore,
                projectGroupStore: projectGroupStore,
                fileSystem: ProjectPathConfirmationFileSystemStub(state: .directory)
            )
        }

        #expect(worktreeStore.list(for: project.id) == existingWorktrees)
        #expect(worktreeStore.primary(for: project.id) == nil)
        #expect(worktreeStore.worktree(projectID: project.id, worktreeID: secondary.id) == secondary)
        #expect(projectStore.storedProjects.isEmpty)
        #expect(projectStore.recentlyRemovedProjects.first?.id == project.id)
    }

    @Test("recent restore is rejected while an SSH workspace is active")
    func recentlyRemovedRestoreRejectsActiveSSHWorkspace() {
        let (appState, projectStore, worktreeStore, _) = makeStores()
        let remoteGroup = ProjectGroup(name: "Remote", type: .ssh)
        let groupPersistence = ProjectGroupPersistenceStub(initial: [remoteGroup])
        let projectGroupStore = ProjectGroupStore(
            persistence: groupPersistence,
            remoteDeviceStore: RemoteDeviceStore(persistence: InMemoryRemoteDevicePersistence()),
            workspaceContextSink: InMemoryWorkspaceContextSink()
        )
        projectGroupStore.selectGroup(id: remoteGroup.id)
        let project = Project(name: "Repo", path: "/tmp/repo")
        projectStore.add(project)
        projectStore.remove(id: project.id)

        #expect(throws: ProjectOpenService.RestoreError.remoteWorkspaceActive) {
            try ProjectOpenService.restoreRecentlyRemovedProject(
                id: project.id,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore,
                projectGroupStore: projectGroupStore,
                fileSystem: ProjectPathConfirmationFileSystemStub(state: .directory)
            )
        }
        #expect(projectStore.storedProjects.isEmpty)
        #expect(projectStore.recentlyRemovedProjects.first?.id == project.id)
        #expect(worktreeStore.primary(for: project.id) == nil)
        #expect(appState.activeProjectID == nil)
        #expect(projectGroupStore.groups.first?.projectIDs.isEmpty == true)
        #expect(groupPersistence.savedGroups == nil)
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

    @Test("remote device project is stored with its device id and selected")
    func remoteDeviceProjectStoredAndSelected() throws {
        let (appState, projectStore, worktreeStore, projectGroupStore, deviceStore) = makeStoresWithDevice()
        let device = deviceStore.add(name: "prod", ssh: SSHWorkspaceData(host: "prod", remoteRoot: "~/code"))

        let result = RemoteDeviceProjectConfirmationService(
            appState: appState, projectStore: projectStore,
            worktreeStore: worktreeStore, projectGroupStore: projectGroupStore
        )
        .confirm(path: "~/code/api", device: device)

        let added = try #require(projectStore.storedProjects.first)
        #expect(result == .success)
        #expect(projectStore.storedProjects.count == 1)
        #expect(added.name == "api")
        #expect(added.remoteDeviceID == device.id)
        #expect(added.isRemote)
        #expect(appState.activeProjectID == added.id)
    }

    @Test("remote device project dedupes by device and standardized path")
    func remoteDeviceProjectDedupes() {
        let (appState, projectStore, worktreeStore, projectGroupStore, deviceStore) = makeStoresWithDevice()
        let device = deviceStore.add(name: "prod", ssh: SSHWorkspaceData(host: "prod", remoteRoot: "~/code"))
        let service = RemoteDeviceProjectConfirmationService(
            appState: appState, projectStore: projectStore,
            worktreeStore: worktreeStore, projectGroupStore: projectGroupStore
        )

        _ = service.confirm(path: "~/code/api", device: device)
        let result = service.confirm(path: "~/code/./api", device: device)

        #expect(result == .success)
        #expect(projectStore.storedProjects.count == 1)
    }

    @Test("remote device project rejects the device root path")
    func remoteDeviceProjectRejectsRoot() {
        let (appState, projectStore, worktreeStore, projectGroupStore, deviceStore) = makeStoresWithDevice()
        let device = deviceStore.add(name: "prod", ssh: SSHWorkspaceData(host: "prod", remoteRoot: "~/code"))

        let result = RemoteDeviceProjectConfirmationService(
            appState: appState, projectStore: projectStore,
            worktreeStore: worktreeStore, projectGroupStore: projectGroupStore
        )
        .confirm(path: "~/code", device: device)

        #expect(result == .failed)
        #expect(projectStore.storedProjects.isEmpty)
    }

    @Test("remote device project is added to the active local group")
    func remoteDeviceProjectAddedToActiveGroup() throws {
        let (appState, projectStore, worktreeStore, _, deviceStore) = makeStoresWithDevice()
        let device = deviceStore.add(name: "prod", ssh: SSHWorkspaceData(host: "prod", remoteRoot: "~/code"))
        let group = ProjectGroup(name: "Work")
        let groupPersistence = ProjectGroupPersistenceStub(initial: [group])
        let projectGroupStore = ProjectGroupStore(persistence: groupPersistence, remoteDeviceStore: deviceStore, workspaceContextSink: InMemoryWorkspaceContextSink())
        projectGroupStore.selectGroup(id: group.id)

        _ = RemoteDeviceProjectConfirmationService(
            appState: appState, projectStore: projectStore,
            worktreeStore: worktreeStore, projectGroupStore: projectGroupStore
        )
        .confirm(path: "~/code/api", device: device)

        let added = try #require(projectStore.storedProjects.first)
        #expect(groupPersistence.savedGroups?.first?.projectIDs == [added.id])
    }

    private func makeStores() -> (AppState, ProjectStore, WorktreeStore, ProjectGroupStore) {
        let (appState, projectStore, worktreeStore, projectGroupStore, _) = makeStoresWithDevice()
        return (appState, projectStore, worktreeStore, projectGroupStore)
    }

    private func makeStoresWithDevice() -> (AppState, ProjectStore, WorktreeStore, ProjectGroupStore, RemoteDeviceStore) {
        let projectStore = ProjectStore(persistence: ProjectPersistenceStub())
        let worktreeStore = WorktreeStore(persistence: WorktreePersistenceStub(), projects: [])
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let deviceStore = RemoteDeviceStore(persistence: InMemoryRemoteDevicePersistence())
        let projectGroupStore = ProjectGroupStore(persistence: ProjectGroupPersistenceStub(), remoteDeviceStore: deviceStore, workspaceContextSink: InMemoryWorkspaceContextSink())
        return (appState, projectStore, worktreeStore, projectGroupStore, deviceStore)
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
    struct SaveError: Error {}

    private var projects: [Project] = []
    private var recentlyRemovedProjects: [RecentlyRemovedProject] = []
    var projectSaveError: Error?

    func loadProjects() throws -> [Project] { projects }
    func saveProjects(_ projects: [Project]) throws {
        if let projectSaveError { throw projectSaveError }
        self.projects = projects
    }

    func loadRecentlyRemovedProjects() throws -> [RecentlyRemovedProject] {
        recentlyRemovedProjects
    }

    func saveRecentlyRemovedProjects(_ projects: [RecentlyRemovedProject]) throws {
        recentlyRemovedProjects = projects
    }
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
