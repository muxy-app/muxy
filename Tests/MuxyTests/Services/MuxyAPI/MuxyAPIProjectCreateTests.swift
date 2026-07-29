import Foundation
import Testing

@testable import Muxy

@Suite("MuxyAPI project create/attach/detach routing")
@MainActor
struct MuxyAPIProjectWorkspaceRoutingTests {
    @Test("create adds a project for an existing directory")
    func createExistingDirectory() async throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("muxy-create-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let env = ProjectManagementEnvironment()

        let result = await MuxyAPI.Projects.create(
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
    func createMissingDirectory() async throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("muxy-create-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: dir) }
        let env = ProjectManagementEnvironment()

        let result = await MuxyAPI.Projects.create(
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
    func createFailsWhenMissing() async {
        let env = ProjectManagementEnvironment()

        let result = await MuxyAPI.Projects.create(
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
    func createRenamesProject() async throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("muxy-create-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let env = ProjectManagementEnvironment()

        let result = await MuxyAPI.Projects.create(
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
    func createAddsToWorkspace() async throws {
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

        let result = await MuxyAPI.Projects.create(
            CreateProjectRequest(path: dir.path, createIfMissing: false, name: nil, workspaceIdentifier: "Work"),
            appState: env.appState,
            projectStore: env.projectStore,
            worktreeStore: env.worktreeStore,
            projectGroupStore: groupStore
        )

        let info = try #require(try? result.get())
        #expect(groupStore.groups.first?.projectIDs == [info.id])
    }

    @Test("create succeeds when workspaceIdentifier is empty")
    func createSucceedsWhenWorkspaceIdentifierEmpty() async throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("muxy-create-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let env = ProjectManagementEnvironment()

        let result = await MuxyAPI.Projects.create(
            CreateProjectRequest(path: dir.path, createIfMissing: false, name: nil, workspaceIdentifier: ""),
            appState: env.appState,
            projectStore: env.projectStore,
            worktreeStore: env.worktreeStore,
            projectGroupStore: env.projectGroupStore
        )

        #expect(result.isSuccess)
    }

    @Test("create fails when workspaceIdentifier does not match any workspace")
    func createFailsWhenWorkspaceMissing() async throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("muxy-create-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: dir) }
        let env = ProjectManagementEnvironment()

        let result = await MuxyAPI.Projects.create(
            CreateProjectRequest(
                path: dir.path,
                createIfMissing: true,
                name: nil,
                workspaceIdentifier: "missing"
            ),
            appState: env.appState,
            projectStore: env.projectStore,
            worktreeStore: env.worktreeStore,
            projectGroupStore: env.projectGroupStore
        )

        guard case .failure(.invalidArguments) = result else {
            Issue.record("expected invalidArguments for a workspace that does not exist")
            return
        }
        #expect(!FileManager.default.fileExists(atPath: dir.path))
        #expect(!env.projectStore.projects.contains { $0.path == dir.path })
    }

    @Test("create fails when the workspace is a remote SSH workspace")
    func createFailsWhenWorkspaceIsRemote() async throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("muxy-create-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: dir) }
        let device = RemoteDevice(name: "Prod", ssh: SSHWorkspaceData(host: "example.com"))
        let env = ProjectManagementEnvironment()
        let groupStore = ProjectGroupStore(
            persistence: ProjectGroupPersistenceStub(),
            remoteDeviceStore: RemoteDeviceStore(persistence: InMemoryRemoteDevicePersistence(initial: [device])),
            workspaceContextSink: InMemoryWorkspaceContextSink()
        )
        let sshGroup = groupStore.addRemoteWorkspace(name: "Remote", deviceID: device.id)

        let result = await MuxyAPI.Projects.create(
            CreateProjectRequest(path: dir.path, createIfMissing: true, name: nil, workspaceIdentifier: sshGroup.name),
            appState: env.appState,
            projectStore: env.projectStore,
            worktreeStore: env.worktreeStore,
            projectGroupStore: groupStore
        )

        guard case .failure(.invalidArguments) = result else {
            Issue.record("expected invalidArguments when attaching to a remote SSH workspace")
            return
        }
        #expect(!FileManager.default.fileExists(atPath: dir.path))
        #expect(!env.projectStore.projects.contains { $0.path == dir.path })
    }

    @Test("create does not override worktreesEnabled on an existing project")
    func createPreservesExistingWorktreesSetting() async throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("muxy-create-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let existing = Project(name: "Existing", path: dir.standardizedFileURL.path)
        let env = ProjectManagementEnvironment(projects: [existing])

        let result = await MuxyAPI.Projects.create(
            CreateProjectRequest(path: dir.path, createIfMissing: false, name: nil, workspaceIdentifier: nil),
            appState: env.appState,
            projectStore: env.projectStore,
            worktreeStore: env.worktreeStore,
            projectGroupStore: env.projectGroupStore
        )

        let info = try #require(try? result.get())
        #expect(info.id == existing.id)
        #expect(env.projectStore.storedProjects.first { $0.id == existing.id }?.worktreesEnabled == false)
    }

    @Test("create requires consent before creating a directory for an extension caller")
    func createRequiresConsentForDirectoryCreation() async {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("muxy-create-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: dir) }
        let env = ProjectManagementEnvironment()

        let result = await MuxyAPI.Projects.create(
            CreateProjectRequest(path: dir.path, createIfMissing: true, name: nil, workspaceIdentifier: nil),
            appState: env.appState,
            projectStore: env.projectStore,
            worktreeStore: env.worktreeStore,
            projectGroupStore: env.projectGroupStore,
            callingExtensionID: "demo",
            consent: makeConsent(extensionID: "demo", decision: .deny)
        )

        guard case .failure(.consentDenied) = result else {
            Issue.record("expected consentDenied when the extension is not allowed to create directories")
            return
        }
        #expect(!FileManager.default.fileExists(atPath: dir.path))
        #expect(!env.projectStore.projects.contains { $0.path == dir.path })
    }

    @Test("create makes the directory for an extension caller once consent is granted")
    func createProceedsWhenConsentGranted() async throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("muxy-create-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: dir) }
        let env = ProjectManagementEnvironment()

        let result = await MuxyAPI.Projects.create(
            CreateProjectRequest(path: dir.path, createIfMissing: true, name: nil, workspaceIdentifier: nil),
            appState: env.appState,
            projectStore: env.projectStore,
            worktreeStore: env.worktreeStore,
            projectGroupStore: env.projectGroupStore,
            callingExtensionID: "demo",
            consent: makeConsent(extensionID: "demo", decision: .allow)
        )

        #expect(result.isSuccess)
        #expect(FileManager.default.fileExists(atPath: dir.path))
    }

    @Test("create does not ask for consent when the directory already exists")
    func createSkipsConsentForExistingDirectory() async throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("muxy-create-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let env = ProjectManagementEnvironment()

        let result = await MuxyAPI.Projects.create(
            CreateProjectRequest(path: dir.path, createIfMissing: true, name: nil, workspaceIdentifier: nil),
            appState: env.appState,
            projectStore: env.projectStore,
            worktreeStore: env.worktreeStore,
            projectGroupStore: env.projectGroupStore,
            callingExtensionID: "demo",
            consent: makeConsent(extensionID: "demo", decision: .deny)
        )

        #expect(result.isSuccess)
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

    @Test("attach fails when the workspace is a remote SSH workspace")
    func attachFailsWhenWorkspaceIsRemote() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let device = RemoteDevice(name: "Prod", ssh: SSHWorkspaceData(host: "example.com"))
        let env = ProjectManagementEnvironment(projects: [project])
        let groupStore = ProjectGroupStore(
            persistence: ProjectGroupPersistenceStub(),
            remoteDeviceStore: RemoteDeviceStore(persistence: InMemoryRemoteDevicePersistence(initial: [device])),
            workspaceContextSink: InMemoryWorkspaceContextSink()
        )
        let sshGroup = groupStore.addRemoteWorkspace(name: "Remote", deviceID: device.id)

        let result = MuxyAPI.Projects.attach(
            projectIdentifier: project.name,
            workspaceIdentifier: sshGroup.name,
            projectStore: env.projectStore,
            projectGroupStore: groupStore,
            appState: env.appState
        )

        guard case .failure(.invalidArguments) = result else {
            Issue.record("expected invalidArguments when attaching to a remote SSH workspace")
            return
        }
    }

    @Test("attach fails for the home project")
    func attachFailsForHomeProject() {
        let home = Project(id: Project.homeID, name: Project.homeName, path: "/tmp/home")
        let group = ProjectGroup(name: "Work")
        let env = ProjectManagementEnvironment(projects: [home])
        let groupStore = ProjectGroupStore(
            persistence: ProjectGroupPersistenceStub(initial: [group]),
            remoteDeviceStore: RemoteDeviceStore(persistence: InMemoryRemoteDevicePersistence()),
            workspaceContextSink: InMemoryWorkspaceContextSink()
        )

        let result = MuxyAPI.Projects.attach(
            projectIdentifier: home.id.uuidString,
            workspaceIdentifier: "Work",
            projectStore: env.projectStore,
            projectGroupStore: groupStore,
            appState: env.appState
        )

        guard case .failure(.invalidArguments) = result else {
            Issue.record("expected invalidArguments when attaching the home project")
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

    @Test("detach fails for the home project")
    func detachFailsForHomeProject() {
        let home = Project(id: Project.homeID, name: Project.homeName, path: "/tmp/home")
        let env = ProjectManagementEnvironment(projects: [home])

        let result = MuxyAPI.Projects.detach(
            projectIdentifier: home.id.uuidString,
            projectStore: env.projectStore,
            projectGroupStore: env.projectGroupStore,
            appState: env.appState
        )

        guard case .failure(.invalidArguments) = result else {
            Issue.record("expected invalidArguments when detaching the home project")
            return
        }
    }

    private func makeConsent(extensionID: String, decision: ExtensionGrantDecision) -> ExtensionConsentService {
        let grantStore = ExtensionGrantStore(fileURL: FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-grants-\(UUID().uuidString).json"))
        grantStore.add(ExtensionGrantRule(
            extensionID: extensionID,
            verb: .filesWrite,
            match: .fileOperationEquals("mkdir"),
            decision: decision
        ))
        return ExtensionConsentService(grantStore: grantStore)
    }
}

extension Result {
    fileprivate var isSuccess: Bool {
        if case .success = self { return true }
        return false
    }
}
