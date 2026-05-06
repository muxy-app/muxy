import Foundation
import MuxyShared
import Testing

@testable import Muxy

@Suite("VCSWorktreeAutoRefresher")
@MainActor
struct VCSWorktreeAutoRefresherTests {
    @Test(".vcsDidRefresh triggers refreshFromGit for the matching project")
    func vcsDidRefreshTriggersRefresh() async {
        let context = makeContext()
        await runRefresh(context: context, notification: .vcsDidRefresh)
        await #expect(context.gitService.callCount(forRepoPath: context.project.path) == 1)
        #expect(context.worktreeStore.list(for: context.project.id).contains { $0.path == context.featurePath })
    }

    @Test(".vcsRepoDidChange triggers refreshFromGit for the matching project")
    func vcsRepoDidChangeTriggersRefresh() async {
        let context = makeContext()
        await runRefresh(context: context, notification: .vcsRepoDidChange)
        await #expect(context.gitService.callCount(forRepoPath: context.project.path) == 1)
        #expect(context.worktreeStore.list(for: context.project.id).contains { $0.path == context.featurePath })
    }

    @Test("notification with unknown repoPath is a no-op")
    func unknownRepoPathIsNoOp() async {
        let context = makeContext()
        NotificationCenter.default.post(
            name: .vcsDidRefresh,
            object: nil,
            userInfo: ["repoPath": "/tmp/muxy-test-unknown-\(UUID().uuidString)"]
        )
        await drainMainQueue()
        await #expect(context.gitService.callCount(forRepoPath: context.project.path) == 0)
        #expect(context.worktreeStore.list(for: context.project.id).count == 1)
    }

    private func runRefresh(context: Context, notification: Notification.Name) async {
        NotificationCenter.default.post(
            name: notification,
            object: nil,
            userInfo: ["repoPath": context.project.path]
        )
        await context.gitService.awaitCall(forRepoPath: context.project.path)
        await drainMainQueue()
    }

    private func drainMainQueue() async {
        await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
            DispatchQueue.main.async {
                DispatchQueue.main.async {
                    continuation.resume()
                }
            }
        }
    }

    private func makeContext() -> Context {
        let suffix = UUID().uuidString
        let projectPath = "/tmp/muxy-test-repo-\(suffix)"
        let featurePath = "/tmp/muxy-test-repo-\(suffix)-feature-x"
        let project = Project(name: "Repo", path: projectPath)
        let projectStore = ProjectStore(persistence: ProjectPersistenceStub(initial: [project]))
        let gitService = TrackingGitWorktreeListingStub(recordsByRepoPath: [
            projectPath: [
                GitWorktreeRecord(
                    path: projectPath,
                    branch: "main",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
                GitWorktreeRecord(
                    path: featurePath,
                    branch: "feature-x",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
            ]
        ])
        let worktreeStore = WorktreeStore(
            persistence: WorktreePersistenceStub(initial: [
                project.id: [Worktree(name: project.name, path: project.path, isPrimary: true)]
            ]),
            listGitWorktrees: gitService.listWorktrees,
            projects: [project]
        )
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let refresher = VCSWorktreeAutoRefresher(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore
        )
        return Context(
            project: project,
            featurePath: featurePath,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            appState: appState,
            gitService: gitService,
            refresher: refresher
        )
    }

    @MainActor
    private final class Context {
        let project: Project
        let featurePath: String
        let projectStore: ProjectStore
        let worktreeStore: WorktreeStore
        let appState: AppState
        let gitService: TrackingGitWorktreeListingStub
        let refresher: VCSWorktreeAutoRefresher

        init(
            project: Project,
            featurePath: String,
            projectStore: ProjectStore,
            worktreeStore: WorktreeStore,
            appState: AppState,
            gitService: TrackingGitWorktreeListingStub,
            refresher: VCSWorktreeAutoRefresher
        ) {
            self.project = project
            self.featurePath = featurePath
            self.projectStore = projectStore
            self.worktreeStore = worktreeStore
            self.appState = appState
            self.gitService = gitService
            self.refresher = refresher
        }
    }
}

private actor GitWorktreeCallTracker {
    private let recordsByRepoPath: [String: [GitWorktreeRecord]]
    private var calls: [String: Int] = [:]
    private var pendingContinuations: [String: [CheckedContinuation<Void, Never>]] = [:]

    init(recordsByRepoPath: [String: [GitWorktreeRecord]]) {
        self.recordsByRepoPath = recordsByRepoPath
    }

    func record(repoPath: String) -> [GitWorktreeRecord] {
        calls[repoPath, default: 0] += 1
        let waiters = pendingContinuations.removeValue(forKey: repoPath) ?? []
        for continuation in waiters {
            continuation.resume()
        }
        return recordsByRepoPath[repoPath] ?? []
    }

    func callCount(forRepoPath repoPath: String) -> Int {
        calls[repoPath, default: 0]
    }

    func awaitCall(forRepoPath repoPath: String) async {
        if calls[repoPath, default: 0] > 0 { return }
        await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
            pendingContinuations[repoPath, default: []].append(continuation)
        }
    }
}

private final class TrackingGitWorktreeListingStub: Sendable {
    let tracker: GitWorktreeCallTracker

    init(recordsByRepoPath: [String: [GitWorktreeRecord]]) {
        self.tracker = GitWorktreeCallTracker(recordsByRepoPath: recordsByRepoPath)
    }

    @Sendable
    func listWorktrees(repoPath: String) async throws -> [GitWorktreeRecord] {
        await tracker.record(repoPath: repoPath)
    }

    func callCount(forRepoPath repoPath: String) async -> Int {
        await tracker.callCount(forRepoPath: repoPath)
    }

    func awaitCall(forRepoPath repoPath: String) async {
        await tracker.awaitCall(forRepoPath: repoPath)
    }
}

private final class ProjectPersistenceStub: ProjectPersisting {
    private var projects: [Project]

    init(initial: [Project] = []) {
        projects = initial
    }

    func loadProjects() throws -> [Project] { projects }
    func saveProjects(_ projects: [Project]) throws { self.projects = projects }
}

private final class WorktreePersistenceStub: WorktreePersisting {
    private var storage: [UUID: [Worktree]]

    init(initial: [UUID: [Worktree]] = [:]) {
        storage = initial
    }

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
