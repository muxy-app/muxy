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

        let tracked = MuxyAPI.Git.trackedWorktree(path: prWorktree.path, project: project, context: context)

        #expect(tracked?.worktree.id == prWorktree.id)
        #expect(tracked?.project.path == project.path)
    }

    @Test("the primary worktree never resolves for removal")
    func primaryWorktreeIsNotRemovable() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let context = makeContext(project: project, worktrees: [primary])

        #expect(MuxyAPI.Git.trackedWorktree(path: primary.path, project: project, context: context) == nil)
    }

    @Test("an untracked path does not resolve, leaving the git fallback to handle it")
    func untrackedPathDoesNotResolve() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let context = makeContext(project: project, worktrees: [primary])

        #expect(MuxyAPI.Git.trackedWorktree(path: "/tmp/repo-unknown", project: project, context: context) == nil)
    }

    @Test("a project cannot resolve another project's worktree")
    func projectCannotResolveAnotherProjectsWorktree() {
        let project = Project(name: "Repo A", path: "/tmp/repo-a")
        let otherProject = Project(name: "Repo B", path: "/tmp/repo-b")
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let otherWorktree = Worktree(
            name: "Feature B",
            path: "/tmp/repo-b-feature",
            branch: "feature-b",
            source: .muxy,
            isPrimary: false
        )
        let context = makeContext(
            projects: [project, otherProject],
            worktrees: [project.id: [primary], otherProject.id: [otherWorktree]]
        )

        let tracked = MuxyAPI.Git.trackedWorktree(
            path: otherWorktree.path,
            project: project,
            context: context
        )

        #expect(tracked == nil)
    }

    @Test("remote stored paths resolve home and repository-relative forms")
    func remoteStoredPathsResolveForTracking() {
        let homeProject = Project(name: "Home Repo", path: "~/repo")
        let homeWorktree = Worktree(name: "Feature", path: "~/repo-feature", isPrimary: false)
        let homeContext = makeContext(project: homeProject, worktrees: [homeWorktree])
        let remote = WorkspaceContext.ssh(SSHDestination(host: "example.com"))

        let homeTracked = MuxyAPI.Git.trackedWorktree(
            path: "/home/test/repo-feature",
            project: homeProject,
            context: homeContext,
            workspaceContext: remote,
            remoteHomePath: "/home/test"
        )

        let relativeProject = Project(name: "Relative Repo", path: "/srv/repos/repo")
        let relativeWorktree = Worktree(name: "Feature", path: "../repo-feature", isPrimary: false)
        let relativeContext = makeContext(project: relativeProject, worktrees: [relativeWorktree])
        let relativeTracked = MuxyAPI.Git.trackedWorktree(
            path: "/srv/repos/repo-feature",
            project: relativeProject,
            context: relativeContext,
            workspaceContext: remote,
            remoteHomePath: "/home/test"
        )

        #expect(homeTracked?.worktree.id == homeWorktree.id)
        #expect(relativeTracked?.worktree.id == relativeWorktree.id)
    }

    @Test("worktree removal rejects timeouts outside the supported range")
    func removalRejectsInvalidTimeouts() async {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let context = makeContext(project: project, worktrees: [primary])

        let zero = await MuxyAPI.Git.removeWorktree(
            projectIdentifier: project.id.uuidString,
            path: "/tmp/worktree",
            force: true,
            timeoutMs: 0,
            context: context
        )
        let excessive = await MuxyAPI.Git.removeWorktree(
            projectIdentifier: project.id.uuidString,
            path: "/tmp/worktree",
            force: true,
            timeoutMs: MuxyAPI.Git.maxWorktreeRemovalTimeoutMs + 1,
            context: context
        )

        let expected = APIError.invalidArguments(
            "timeoutMs must be between 1 and \(MuxyAPI.Git.maxWorktreeRemovalTimeoutMs)"
        )
        #expect(zero == .failure(expected))
        #expect(excessive == .failure(expected))
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

    private func makeContext(project: Project, worktrees: [Worktree]) -> MuxyAPI.Git.Context {
        makeContext(projects: [project], worktrees: [project.id: worktrees])
    }

    private func makeContext(
        projects: [Project],
        worktrees: [UUID: [Worktree]]
    ) -> MuxyAPI.Git.Context {
        let projectStore = ProjectStore(persistence: ProjectPersistenceStub())
        for project in projects {
            projectStore.add(project)
        }
        let worktreeStore = WorktreeStore(
            persistence: WorktreePersistenceStub(initial: worktrees),
            projects: projects
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
