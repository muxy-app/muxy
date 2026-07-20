import Foundation
import Testing

@testable import Muxy

@Suite("MuxyAPI project create/attach/detach routing")
@MainActor
struct MuxyAPIProjectWorkspaceRoutingTests {
    @Test("create adds a project for an existing directory")
    func createExistingDirectory() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("muxy-create-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let env = ProjectManagementEnvironment()

        let result = MuxyAPI.Projects.create(
            CreateProjectRequest(path: dir.path, createIfMissing: false, name: nil, workspaceIdentifier: nil),
            appState: env.appState,
            projectStore: env.projectStore,
            worktreeStore: env.worktreeStore,
            projectGroupStore: env.projectGroupStore
        )

        let info = try #require(try? result.get())
        #expect(info.name == dir.lastPathComponent)
        #expect(info.path == dir.path)
        #expect(env.projectStore.projects.contains { $0.id == info.id })
        #expect(env.projectStore.storedProjects.first { $0.id == info.id }?.worktreesEnabled == true)
    }

    @Test("create creates a directory when createIfMissing is true")
    func createMissingDirectory() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("muxy-create-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: dir) }
        let env = ProjectManagementEnvironment()

        let result = MuxyAPI.Projects.create(
            CreateProjectRequest(path: dir.path, createIfMissing: true, name: nil, workspaceIdentifier: nil),
            appState: env.appState,
            projectStore: env.projectStore,
            worktreeStore: env.worktreeStore,
            projectGroupStore: env.projectGroupStore
        )

        #expect(FileManager.default.fileExists(atPath: dir.path))
        #expect(result.isSuccess)
    }

    @Test("create fails when directory is missing and createIfMissing is false")
    func createFailsWhenMissing() {
        let env = ProjectManagementEnvironment()

        let result = MuxyAPI.Projects.create(
            CreateProjectRequest(
                path: "/tmp/muxy-missing-\(UUID().uuidString)",
                createIfMissing: false,
                name: nil,
                workspaceIdentifier: nil
            ),
            appState: env.appState,
            projectStore: env.projectStore,
            worktreeStore: env.worktreeStore,
            projectGroupStore: env.projectGroupStore
        )

        guard case .failure(.invalidArguments) = result else {
            Issue.record("expected invalidArguments for missing directory")
            return
        }
    }

    @Test("create renames project when name is provided")
    func createRenamesProject() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("muxy-create-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let env = ProjectManagementEnvironment()

        let result = MuxyAPI.Projects.create(
            CreateProjectRequest(path: dir.path, createIfMissing: false, name: "Custom Name", workspaceIdentifier: nil),
            appState: env.appState,
            projectStore: env.projectStore,
            worktreeStore: env.worktreeStore,
            projectGroupStore: env.projectGroupStore
        )

        let info = try #require(try? result.get())
        #expect(info.name == "Custom Name")
        #expect(env.projectStore.storedProjects.first { $0.id == info.id }?.name == "Custom Name")
    }

    @Test("create adds project to workspace when workspaceIdentifier is provided")
    func createAddsToWorkspace() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("muxy-create-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let group = ProjectGroup(name: "Work")
        let env = ProjectManagementEnvironment()
        let groupStore = ProjectGroupStore(
            persistence: ProjectGroupPersistenceStub(initial: [group]),
            remoteDeviceStore: RemoteDeviceStore(persistence: InMemoryRemoteDevicePersistence()),
            workspaceContextSink: InMemoryWorkspaceContextSink()
        )

        let result = MuxyAPI.Projects.create(
            CreateProjectRequest(path: dir.path, createIfMissing: false, name: nil, workspaceIdentifier: "Work"),
            appState: env.appState,
            projectStore: env.projectStore,
            worktreeStore: env.worktreeStore,
            projectGroupStore: groupStore
        )

        let info = try #require(try? result.get())
        #expect(groupStore.groups.first?.projectIDs == [info.id])
    }

    @Test("create succeeds even when workspaceIdentifier does not match any workspace")
    func createSucceedsWhenWorkspaceMissing() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("muxy-create-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let env = ProjectManagementEnvironment()

        let result = MuxyAPI.Projects.create(
            CreateProjectRequest(
                path: dir.path,
                createIfMissing: false,
                name: nil,
                workspaceIdentifier: "missing"
            ),
            appState: env.appState,
            projectStore: env.projectStore,
            worktreeStore: env.worktreeStore,
            projectGroupStore: env.projectGroupStore
        )

        #expect(result.isSuccess)
        #expect(env.projectGroupStore.groups.isEmpty)
    }

    @Test("attach adds project to workspace")
    func attach() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let group = ProjectGroup(name: "Work")
        let env = ProjectManagementEnvironment(projects: [project])
        let groupStore = ProjectGroupStore(
            persistence: ProjectGroupPersistenceStub(initial: [group]),
            remoteDeviceStore: RemoteDeviceStore(persistence: InMemoryRemoteDevicePersistence()),
            workspaceContextSink: InMemoryWorkspaceContextSink()
        )

        let result = MuxyAPI.Projects.attach(
            projectIdentifier: project.name,
            workspaceIdentifier: "Work",
            projectStore: env.projectStore,
            projectGroupStore: groupStore,
            appState: env.appState
        )

        #expect(result.isSuccess)
        #expect(groupStore.groups.first?.projectIDs == [project.id])
    }

    @Test("attach fails when project is not found")
    func attachFailsWhenProjectMissing() {
        let group = ProjectGroup(name: "Work")
        let env = ProjectManagementEnvironment()
        let groupStore = ProjectGroupStore(
            persistence: ProjectGroupPersistenceStub(initial: [group]),
            remoteDeviceStore: RemoteDeviceStore(persistence: InMemoryRemoteDevicePersistence()),
            workspaceContextSink: InMemoryWorkspaceContextSink()
        )

        let result = MuxyAPI.Projects.attach(
            projectIdentifier: "missing",
            workspaceIdentifier: "Work",
            projectStore: env.projectStore,
            projectGroupStore: groupStore,
            appState: env.appState
        )

        guard case .failure(.projectNotFound) = result else {
            Issue.record("expected projectNotFound for missing project")
            return
        }
    }

    @Test("attach fails when workspace is not found")
    func attachFailsWhenWorkspaceMissing() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let env = ProjectManagementEnvironment(projects: [project])

        let result = MuxyAPI.Projects.attach(
            projectIdentifier: project.name,
            workspaceIdentifier: "missing",
            projectStore: env.projectStore,
            projectGroupStore: env.projectGroupStore,
            appState: env.appState
        )

        guard case .failure(.invalidArguments) = result else {
            Issue.record("expected invalidArguments for missing workspace")
            return
        }
    }

    @Test("detach removes project from all workspaces")
    func detach() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let group = ProjectGroup(name: "Work", projectIDs: [project.id])
        let env = ProjectManagementEnvironment(projects: [project])
        let groupStore = ProjectGroupStore(
            persistence: ProjectGroupPersistenceStub(initial: [group]),
            remoteDeviceStore: RemoteDeviceStore(persistence: InMemoryRemoteDevicePersistence()),
            workspaceContextSink: InMemoryWorkspaceContextSink()
        )

        let result = MuxyAPI.Projects.detach(
            projectIdentifier: project.name,
            projectStore: env.projectStore,
            projectGroupStore: groupStore,
            appState: env.appState
        )

        #expect(result.isSuccess)
        #expect(groupStore.groups.first?.projectIDs.isEmpty == true)
    }

    @Test("detach fails when project is not found")
    func detachFailsWhenProjectMissing() {
        let env = ProjectManagementEnvironment()

        let result = MuxyAPI.Projects.detach(
            projectIdentifier: "missing",
            projectStore: env.projectStore,
            projectGroupStore: env.projectGroupStore,
            appState: env.appState
        )

        guard case .failure(.projectNotFound) = result else {
            Issue.record("expected projectNotFound for missing project")
            return
        }
    }
}

extension Result {
    fileprivate var isSuccess: Bool {
        if case .success = self { return true }
        return false
    }
}
