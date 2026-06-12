import Foundation
import Testing

@testable import Muxy

@Suite("MuxyAPI.Git worktree removal")
@MainActor
struct MuxyAPIGitWorktreeTests {
    @Test("a removed worktree resolves to its project so teardown runs against the primary repo")
    func resolvesTrackedWorktreeForCleanup() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let prWorktree = Worktree(
            name: "PR 42",
            path: "/tmp/repo-pr-42",
            branch: "pr-42",
            source: .muxy,
            isPrimary: false
        )
        let context = makeContext(project: project, worktrees: [primary, prWorktree])

        let tracked = MuxyAPI.Git.trackedWorktree(path: prWorktree.path, context: context)

        #expect(tracked?.worktree.id == prWorktree.id)
        #expect(tracked?.project.path == project.path)
    }

    @Test("the primary worktree never resolves for removal")
    func primaryWorktreeIsNotRemovable() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let context = makeContext(project: project, worktrees: [primary])

        #expect(MuxyAPI.Git.trackedWorktree(path: primary.path, context: context) == nil)
    }

    @Test("an untracked path does not resolve, leaving the git fallback to handle it")
    func untrackedPathDoesNotResolve() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let context = makeContext(project: project, worktrees: [primary])

        #expect(MuxyAPI.Git.trackedWorktree(path: "/tmp/repo-unknown", context: context) == nil)
    }

    @Test("forgetting a worktree drops it and switches to the replacement")
    func forgetSwitchesToReplacement() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let prWorktree = Worktree(
            name: "PR 42",
            path: "/tmp/repo-pr-42",
            branch: "pr-42",
            source: .muxy,
            isPrimary: false
        )
        let context = makeContext(project: project, worktrees: [primary, prWorktree])
        context.appState.selectProject(project, worktree: prWorktree)

        MuxyAPI.Git.forgetWorktree(project: project, worktree: prWorktree, context: context)

        let remaining = context.worktreeStore.list(for: project.id)
        #expect(!remaining.contains { $0.id == prWorktree.id })
        #expect(context.appState.activeWorktreeID[project.id] == primary.id)
    }

    @Test("projects list includes remote projects outside the local store")
    func projectsListIncludesRemoteProjects() throws {
        let deviceStore = RemoteDeviceStore(persistence: InMemoryRemoteDevicePersistence())
        let device = deviceStore.add(name: "prod", ssh: SSHWorkspaceData(host: "prod", remoteRoot: "~/code"))
        let groupStore = ProjectGroupStore(
            persistence: ProjectGroupPersistenceStub(),
            remoteDeviceStore: deviceStore,
            workspaceContextSink: InMemoryWorkspaceContextSink()
        )
        let group = groupStore.addRemoteWorkspace(name: "prod", deviceID: device.id)
        let remote = try #require(groupStore.addRemoteProject(name: "api", path: "~/code/api", toGroup: group.id))
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let projectStore = ProjectStore(persistence: ProjectPersistenceStub())

        let listed = MuxyAPI.Projects.list(
            appState: appState,
            projectStore: projectStore,
            projectGroupStore: groupStore
        )

        #expect(listed.contains { $0.id == remote.id && $0.path == "~/code/api" })
    }

    @Test("worktrees list resolves the active remote project")
    func worktreesListResolvesActiveRemoteProject() throws {
        let projectStore = ProjectStore(persistence: ProjectPersistenceStub())
        let deviceStore = RemoteDeviceStore(persistence: InMemoryRemoteDevicePersistence())
        let device = deviceStore.add(name: "prod", ssh: SSHWorkspaceData(host: "prod", remoteRoot: "~/code"))
        let groupStore = ProjectGroupStore(
            persistence: ProjectGroupPersistenceStub(),
            remoteDeviceStore: deviceStore,
            workspaceContextSink: InMemoryWorkspaceContextSink()
        )
        let group = groupStore.addRemoteWorkspace(name: "prod", deviceID: device.id)
        let remote = try #require(groupStore.addRemoteProject(name: "api", path: "~/code/api", toGroup: group.id))
            .asProject(workspaceID: group.id, sortOrder: 0)
        let primary = Worktree(name: remote.name, path: remote.path, isPrimary: true)
        let worktreeStore = WorktreeStore(
            persistence: WorktreePersistenceStub(initial: [remote.id: [primary]]),
            projects: [remote]
        )
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        appState.selectProject(remote, worktree: primary)

        let listed = try unwrap(MuxyAPI.Worktrees.list(
            projectIdentifier: nil,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: groupStore
        ))

        #expect(listed.count == 1)
        #expect(listed.first?.path == "~/code/api")
    }

    private func makeContext(project: Project, worktrees: [Worktree]) -> MuxyAPI.Git.Context {
        let projectStore = ProjectStore(persistence: ProjectPersistenceStub())
        projectStore.add(project)
        let worktreeStore = WorktreeStore(
            persistence: WorktreePersistenceStub(initial: [project.id: worktrees]),
            projects: [project]
        )
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
        return MuxyAPI.Git.Context(
            extensionID: "test",
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )
    }
}

private func unwrap<T>(_ result: Result<T, APIError>) throws -> T {
    switch result {
    case let .success(value):
        return value
    case let .failure(error):
        throw error
    }
}

private final class ProjectPersistenceStub: ProjectPersisting {
    private var projects: [Project] = []
    func loadProjects() throws -> [Project] { projects }
    func saveProjects(_ projects: [Project]) throws { self.projects = projects }
}

private final class WorktreePersistenceStub: WorktreePersisting {
    private var storage: [UUID: [Worktree]]
    init(initial: [UUID: [Worktree]]) { storage = initial }
    func loadWorktrees(projectID: UUID) throws -> [Worktree] { storage[projectID] ?? [] }
    func saveWorktrees(_ worktrees: [Worktree], projectID: UUID) throws { storage[projectID] = worktrees }
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
