import Foundation
import Testing

@testable import Muxy

@Suite("MuxyAPI project deletion permissions")
struct MuxyAPIProjectDeletePermissionTests {
    @Test("projects.delete requires projects:delete")
    func deleteRequiresProjectsDelete() {
        #expect(MuxyAPI.Permissions.required(for: "projects.delete") == .projectsDelete)
    }

    @Test("projects.delete is a known verb")
    func deleteIsKnownVerb() {
        #expect(MuxyAPI.Permissions.verbNames.contains("projects.delete"))
    }

    @Test("delete consent defaults to remembering the project name")
    func deleteConsentRemembersProjectName() {
        let match = ExtensionGrantSuggestion.defaultRememberMatch(
            verb: .projectsDelete,
            payload: .project(name: "Repo", path: "/tmp/repo")
        )
        #expect(match == .projectNameEquals("Repo"))
    }
}

@Suite("MuxyAPI project deletion routing")
@MainActor
struct MuxyAPIProjectDeleteRoutingTests {
    @Test("home project cannot be deleted")
    func homeProjectRejected() async {
        let env = makeEnvironment(projects: [])
        let result = await MuxyAPI.Projects.delete(
            identifier: Project.homeID.uuidString,
            context: env.context
        )
        guard case .failure(.invalidArguments) = result else {
            Issue.record("expected invalidArguments for the home project")
            return
        }
    }

    @Test("unknown project is rejected")
    func unknownProjectRejected() async {
        let env = makeEnvironment(projects: [])
        let result = await MuxyAPI.Projects.delete(
            identifier: "does-not-exist",
            context: env.context
        )
        guard case .failure(.projectNotFound) = result else {
            Issue.record("expected projectNotFound for an unknown identifier")
            return
        }
    }

    @Test("ProjectRemovalService removes a local project from the stores")
    func removalServiceRemovesLocalProject() async throws {
        let project = Project(name: "Repo", path: "/tmp/muxy-delete-test-\(UUID().uuidString)")
        let env = makeEnvironment(projects: [project])
        #expect(env.projectStore.storedProjects.contains { $0.id == project.id })

        try await ProjectRemovalService.remove(
            project,
            appState: env.appState,
            projectStore: env.projectStore,
            worktreeStore: env.worktreeStore,
            projectGroupStore: env.projectGroupStore
        )

        #expect(env.projectStore.storedProjects.contains { $0.id == project.id } == false)
        #expect(env.projectStore.recentlyRemovedProjects.first?.project.id == project.id)
    }

    @Test("ProjectRemovalService keeps a local project when recent history cannot be saved")
    func removalServicePreservesProjectAfterPersistenceFailure() async {
        let project = Project(name: "Repo", path: "/tmp/muxy-delete-test-\(UUID().uuidString)")
        let env = makeEnvironment(projects: [project])
        let worktreeDirectory = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-removal-worktree-\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: worktreeDirectory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: worktreeDirectory) }
        let worktree = Worktree(
            name: "Feature",
            path: worktreeDirectory.path,
            branch: "feature",
            isPrimary: false
        )
        env.worktreeStore.add(worktree, to: project.id)
        env.projectPersistence.recentlyRemovedSaveError = ProjectDeletePersistenceStub.SaveError()

        await #expect(throws: ProjectRemovalService.RemovalError.self) {
            try await ProjectRemovalService.remove(
                project,
                appState: env.appState,
                projectStore: env.projectStore,
                worktreeStore: env.worktreeStore,
                projectGroupStore: env.projectGroupStore
            )
        }

        #expect(env.projectStore.storedProjects.contains { $0.id == project.id })
        #expect(env.projectStore.recentlyRemovedProjects.isEmpty)
        #expect(env.worktreeStore.list(for: project.id).contains { $0.id == worktree.id })
        #expect(FileManager.default.fileExists(atPath: worktreeDirectory.path))
    }

    @Test("ProjectRemovalService rolls back staged history when worktree cleanup fails")
    func removalServiceRollsBackHistoryAfterCleanupFailure() async {
        let project = Project(name: "Repo", path: "/tmp/muxy-delete-test-\(UUID().uuidString)")
        let env = makeEnvironment(projects: [project])

        await #expect(throws: ProjectRemovalCleanupError.self) {
            try await ProjectRemovalService.removeLocalProjectData(
                project,
                projectStore: env.projectStore,
                worktreeStore: env.worktreeStore,
                cleanupOnDisk: { _, _ in throw ProjectRemovalCleanupError() }
            )
        }

        #expect(env.projectStore.storedProjects.contains { $0.id == project.id })
        #expect(env.projectStore.recentlyRemovedProjects.isEmpty)
        #expect(env.projectPersistence.recentlyRemovedProjects.isEmpty)
    }

    @Test("ProjectRemovalService freezes project and worktree mutations during cleanup")
    func removalServiceFreezesMutationsDuringCleanup() async throws {
        var project = Project(name: "Repo", path: "/tmp/muxy-delete-test-\(UUID().uuidString)")
        project.logo = "logo.png"
        let env = makeEnvironment(projects: [project])
        let secondary = Worktree(
            name: "Feature",
            path: "/tmp/muxy-delete-feature-\(UUID().uuidString)",
            branch: "feature",
            isPrimary: false
        )
        env.worktreeStore.add(secondary, to: project.id)
        let worktreesBeforeRemoval = env.worktreeStore.list(for: project.id)
        let gate = ProjectRemovalTestGate()

        let removal = Task { @MainActor in
            try await ProjectRemovalService.removeLocalProjectData(
                project,
                projectStore: env.projectStore,
                worktreeStore: env.worktreeStore,
                cleanupOnDisk: { _, _ in await gate.enterAndWait() }
            )
        }
        await gate.waitForEntries(1)

        env.projectStore.rename(id: project.id, to: "Changed")
        env.projectStore.setLogo(id: project.id, to: nil)
        env.projectStore.setPinned(id: project.id, to: true)
        env.worktreeStore.rename(worktreeID: secondary.id, in: project.id, to: "Changed")
        env.worktreeStore.remove(worktreeID: secondary.id, from: project.id)
        env.worktreeStore.add(
            Worktree(name: "Late", path: "/tmp/late", branch: "late", isPrimary: false),
            to: project.id
        )

        let activeProject = try #require(env.projectStore.storedProjects.first)
        #expect(activeProject.name == "Repo")
        #expect(activeProject.logo == "logo.png")
        #expect(!activeProject.isPinned)
        #expect(env.worktreeStore.list(for: project.id) == worktreesBeforeRemoval)

        await gate.releaseNext()
        try await removal.value

        let archivedProject = try #require(env.projectStore.recentlyRemovedProjects.first?.project)
        #expect(archivedProject.name == "Repo")
        #expect(archivedProject.logo == "logo.png")
        #expect(!archivedProject.isPinned)
    }

    @Test("ProjectRemovalService serializes overlapping removals")
    func removalServiceSerializesOverlappingRemovals() async throws {
        let firstProject = Project(name: "First", path: "/tmp/muxy-delete-first-\(UUID().uuidString)")
        let secondProject = Project(name: "Second", path: "/tmp/muxy-delete-second-\(UUID().uuidString)")
        let env = makeEnvironment(projects: [firstProject, secondProject])
        let gate = ProjectRemovalTestGate()

        let firstRemoval = Task { @MainActor in
            try await ProjectRemovalService.removeLocalProjectData(
                firstProject,
                projectStore: env.projectStore,
                worktreeStore: env.worktreeStore,
                cleanupOnDisk: { _, _ in await gate.enterAndWait() }
            )
        }
        await gate.waitForEntries(1)

        let secondRemoval = Task { @MainActor in
            try await ProjectRemovalService.removeLocalProjectData(
                secondProject,
                projectStore: env.projectStore,
                worktreeStore: env.worktreeStore,
                cleanupOnDisk: { _, _ in await gate.enterAndWait() }
            )
        }
        for _ in 0 ..< 10 {
            await Task.yield()
        }
        let enteredBeforeRelease = await gate.entryCount
        #expect(enteredBeforeRelease == 1)

        await gate.releaseNext()
        try await firstRemoval.value
        await gate.waitForEntries(2)
        await gate.releaseNext()
        try await secondRemoval.value

        #expect(env.projectStore.storedProjects.isEmpty)
        #expect(env.projectStore.recentlyRemovedProjects.map(\.id) == [secondProject.id, firstProject.id])
    }

    @Test("ProjectRemovalService waits for active worktree creation before removing an SSH workspace project")
    func removalServiceWaitsForSSHWorkspaceMutation() async throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-remote-workspace-removal-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let persistence = WorktreeDeletePersistenceStub()
        let gate = ProjectRemovalTestGate()
        let worktreeStore = WorktreeStore(
            persistence: persistence,
            addGitWorktree: { _, _, _, _, _ in await gate.enterAndWait() }
        )
        let env = makeEnvironment(projects: [], worktreeStore: worktreeStore)
        let group = env.projectGroupStore.addRemoteWorkspace(name: "Remote", deviceID: UUID())
        let remoteProject = try #require(env.projectGroupStore.addRemoteProject(
            name: "Repo",
            path: "~/repo",
            toGroup: group.id
        ))
        let project = remoteProject.asProject(workspaceID: group.id, sortOrder: 0)
        let request = WorktreeCreationRequest(
            name: "Feature",
            path: root.appendingPathComponent("feature").path,
            branch: "feature",
            createBranch: true,
            baseBranch: nil
        )

        let creation = Task { @MainActor in
            try await worktreeStore.createWorktree(project: project, request: request)
        }
        await gate.waitForEntries(1)
        let removal = Task { @MainActor in
            try await ProjectRemovalService.remove(
                project,
                appState: env.appState,
                projectStore: env.projectStore,
                worktreeStore: worktreeStore,
                projectGroupStore: env.projectGroupStore
            )
        }
        for _ in 0 ..< 10 where !worktreeStore.isProjectRemovalInProgress(project.id) {
            await Task.yield()
        }

        #expect(worktreeStore.isProjectRemovalInProgress(project.id))
        #expect(env.projectGroupStore.groups.first?.remoteProjects.contains { $0.id == project.id } == true)

        await gate.releaseNext()
        _ = try await creation.value
        try await removal.value

        #expect(env.projectGroupStore.groups.first?.remoteProjects.contains { $0.id == project.id } == false)
        #expect(worktreeStore.list(for: project.id).isEmpty)
        #expect(try persistence.loadWorktrees(projectID: project.id).isEmpty)
    }

    @Test("ProjectRemovalService waits for active worktree creation before removing a remote-device project")
    func removalServiceWaitsForRemoteDeviceMutation() async throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-remote-device-removal-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let project = Project(
            name: "Repo",
            path: "~/repo",
            remoteDeviceID: UUID()
        )
        let persistence = WorktreeDeletePersistenceStub()
        let gate = ProjectRemovalTestGate()
        let worktreeStore = WorktreeStore(
            persistence: persistence,
            addGitWorktree: { _, _, _, _, _ in await gate.enterAndWait() },
            projects: [project]
        )
        let env = makeEnvironment(projects: [project], worktreeStore: worktreeStore)
        let request = WorktreeCreationRequest(
            name: "Feature",
            path: root.appendingPathComponent("feature").path,
            branch: "feature",
            createBranch: true,
            baseBranch: nil
        )

        let creation = Task { @MainActor in
            try await worktreeStore.createWorktree(project: project, request: request)
        }
        await gate.waitForEntries(1)
        let removal = Task { @MainActor in
            try await ProjectRemovalService.remove(
                project,
                appState: env.appState,
                projectStore: env.projectStore,
                worktreeStore: worktreeStore,
                projectGroupStore: env.projectGroupStore
            )
        }
        for _ in 0 ..< 10 where !worktreeStore.isProjectRemovalInProgress(project.id) {
            await Task.yield()
        }

        #expect(worktreeStore.isProjectRemovalInProgress(project.id))
        #expect(env.projectStore.storedProjects.contains { $0.id == project.id })

        await gate.releaseNext()
        _ = try await creation.value
        try await removal.value

        #expect(env.projectStore.storedProjects.contains { $0.id == project.id } == false)
        #expect(env.projectStore.recentlyRemovedProjects.isEmpty)
        #expect(worktreeStore.list(for: project.id).isEmpty)
        #expect(try persistence.loadWorktrees(projectID: project.id).isEmpty)
    }

    private struct Environment {
        let appState: AppState
        let projectStore: ProjectStore
        let projectPersistence: ProjectDeletePersistenceStub
        let worktreeStore: WorktreeStore
        let projectGroupStore: ProjectGroupStore

        var context: MuxyAPI.Projects.Context {
            MuxyAPI.Projects.Context(
                extensionID: "test",
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore,
                projectGroupStore: projectGroupStore
            )
        }
    }

    private func makeEnvironment(projects: [Project], worktreeStore: WorktreeStore? = nil) -> Environment {
        let projectPersistence = ProjectDeletePersistenceStub(initial: projects)
        let projectStore = ProjectStore(persistence: projectPersistence)
        let worktreeStore = worktreeStore ?? WorktreeStore(
            persistence: WorktreeDeletePersistenceStub(), projects: projects
        )
        let appState = AppState(
            selectionStore: ProjectDeleteSelectionStoreStub(),
            terminalViews: ProjectDeleteTerminalViewRemovingStub(),
            workspacePersistence: ProjectDeleteWorkspacePersistenceStub()
        )
        let projectGroupStore = ProjectGroupStore(
            persistence: ProjectGroupPersistenceStub(),
            remoteDeviceStore: RemoteDeviceStore(persistence: InMemoryRemoteDevicePersistence()),
            workspaceContextSink: InMemoryWorkspaceContextSink()
        )
        return Environment(
            appState: appState,
            projectStore: projectStore,
            projectPersistence: projectPersistence,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )
    }
}

private final class ProjectDeletePersistenceStub: ProjectPersisting {
    struct SaveError: Error {}

    var projects: [Project]
    var recentlyRemovedProjects: [RecentlyRemovedProject] = []
    var recentlyRemovedSaveError: Error?

    init(initial: [Project]) { projects = initial }
    func loadProjects() throws -> [Project] { projects }
    func saveProjects(_ projects: [Project]) throws { self.projects = projects }
    func loadRecentlyRemovedProjects() throws -> [RecentlyRemovedProject] { recentlyRemovedProjects }
    func saveRecentlyRemovedProjects(_ projects: [RecentlyRemovedProject]) throws {
        if let recentlyRemovedSaveError { throw recentlyRemovedSaveError }
        recentlyRemovedProjects = projects
    }
}

private struct ProjectRemovalCleanupError: Error {}

private actor ProjectRemovalTestGate {
    private(set) var entryCount = 0
    private var entryWaiters: [(Int, CheckedContinuation<Void, Never>)] = []
    private var releaseWaiters: [CheckedContinuation<Void, Never>] = []

    func enterAndWait() async {
        entryCount += 1
        let readyWaiters = entryWaiters.filter { $0.0 <= entryCount }
        entryWaiters.removeAll { $0.0 <= entryCount }
        for waiter in readyWaiters {
            waiter.1.resume()
        }
        await withCheckedContinuation { continuation in
            releaseWaiters.append(continuation)
        }
    }

    func waitForEntries(_ count: Int) async {
        guard entryCount < count else { return }
        await withCheckedContinuation { continuation in
            entryWaiters.append((count, continuation))
        }
    }

    func releaseNext() {
        guard !releaseWaiters.isEmpty else { return }
        releaseWaiters.removeFirst().resume()
    }
}

private final class WorktreeDeletePersistenceStub: WorktreePersisting {
    private var storage: [UUID: [Worktree]] = [:]
    func loadWorktrees(projectID: UUID) throws -> [Worktree] { storage[projectID] ?? [] }
    func saveWorktrees(_ worktrees: [Worktree], projectID: UUID) throws { storage[projectID] = worktrees }
    func removeWorktrees(projectID: UUID) throws { storage.removeValue(forKey: projectID) }
}

private final class ProjectDeleteWorkspacePersistenceStub: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
}

@MainActor
private final class ProjectDeleteSelectionStoreStub: ActiveProjectSelectionStoring {
    func loadActiveProjectID() -> UUID? { nil }
    func saveActiveProjectID(_: UUID?) {}
    func loadActiveWorktreeIDs() -> [UUID: UUID] { [:] }
    func saveActiveWorktreeIDs(_: [UUID: UUID]) {}
}

@MainActor
private final class ProjectDeleteTerminalViewRemovingStub: TerminalViewRemoving {
    func removeView(for _: UUID) {}
    func needsConfirmQuit(for _: UUID) -> Bool { false }
}
