import Foundation
import Testing

@testable import Muxy

@Suite("MuxyAPI.Git worktree removal")
@MainActor
struct MuxyAPIGitWorktreeTests {
    @Test("a removed worktree resolves to its project so teardown runs against the primary repo")
    func resolvesTrackedWorktreeForCleanup() async throws {
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

        let tracked = try await MuxyAPI.Git.trackedWorktree(
            path: prWorktree.path,
            project: project,
            context: context
        )

        #expect(tracked?.worktree.id == prWorktree.id)
        #expect(tracked?.project.path == project.path)
    }

    @Test("the primary worktree never resolves for removal")
    func primaryWorktreeIsNotRemovable() async throws {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let context = makeContext(project: project, worktrees: [primary])

        let tracked = try await MuxyAPI.Git.trackedWorktree(
            path: primary.path,
            project: project,
            context: context
        )

        #expect(tracked == nil)
    }

    @Test("an untracked path does not resolve, leaving the git fallback to handle it")
    func untrackedPathDoesNotResolve() async throws {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let context = makeContext(project: project, worktrees: [primary])

        let tracked = try await MuxyAPI.Git.trackedWorktree(
            path: "/tmp/repo-unknown",
            project: project,
            context: context
        )

        #expect(tracked == nil)
    }

    @Test("a project cannot resolve another project's worktree")
    func projectCannotResolveAnotherProjectsWorktree() async throws {
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

        let tracked = try await MuxyAPI.Git.trackedWorktree(
            path: otherWorktree.path,
            project: project,
            context: context
        )

        #expect(tracked == nil)
    }

    @Test("remote stored paths resolve home and repository-relative forms")
    func remoteStoredPathsResolveForTracking() async throws {
        let homeProject = Project(name: "Home Repo", path: "~/repo")
        let homeWorktree = Worktree(name: "Feature", path: "~/repo-feature", isPrimary: false)
        let homeContext = makeContext(project: homeProject, worktrees: [homeWorktree])
        let remote = WorkspaceContext.ssh(SSHDestination(host: "example.com"))
        let homePrimaryResolution = GitWorktreeService.WorktreePathResolution(
            path: "/home/test/repo",
            identityPaths: ["/home/test/repo"],
            remoteHomePath: "/home/test"
        )
        let homeResolution = GitWorktreeService.WorktreePathResolution(
            path: "/home/test/repo-feature",
            identityPaths: ["/home/test/repo-feature"],
            remoteHomePath: "/home/test"
        )

        let homeTracked = try await MuxyAPI.Git.trackedWorktree(
            path: "/home/test/repo-feature",
            project: homeProject,
            context: homeContext,
            workspaceContext: remote,
            remoteHomePath: "/home/test",
            deadline: OperationDeadline(timeout: 10),
            remotePathResolver: { _, _, _, _, _ in [homePrimaryResolution, homeResolution] }
        )

        let relativeProject = Project(name: "Relative Repo", path: "/srv/repos/repo")
        let relativeWorktree = Worktree(name: "Feature", path: "../repo-feature", isPrimary: false)
        let relativeContext = makeContext(project: relativeProject, worktrees: [relativeWorktree])
        let relativePrimaryResolution = GitWorktreeService.WorktreePathResolution(
            path: relativeProject.path,
            identityPaths: [relativeProject.path],
            remoteHomePath: "/home/test"
        )
        let relativeResolution = GitWorktreeService.WorktreePathResolution(
            path: "/srv/repos/repo-feature",
            identityPaths: ["/srv/repos/repo-feature"],
            remoteHomePath: "/home/test"
        )
        let relativeTracked = try await MuxyAPI.Git.trackedWorktree(
            path: "/srv/repos/repo-feature",
            project: relativeProject,
            context: relativeContext,
            workspaceContext: remote,
            remoteHomePath: "/home/test",
            deadline: OperationDeadline(timeout: 10),
            remotePathResolver: { _, _, _, _, _ in
                [relativePrimaryResolution, relativeResolution]
            }
        )

        #expect(homeTracked?.worktree.id == homeWorktree.id)
        #expect(relativeTracked?.worktree.id == relativeWorktree.id)
    }

    @Test("local stored paths resolve relative to their selected repository")
    func localStoredRelativePathsResolveForTracking() async throws {
        let project = Project(name: "Repo", path: "/srv/repos/repo")
        let worktree = Worktree(name: "Feature", path: "../repo-feature", isPrimary: false)
        let context = makeContext(project: project, worktrees: [worktree])

        let tracked = try await MuxyAPI.Git.trackedWorktree(
            path: "/srv/repos/repo-feature",
            project: project,
            context: context
        )

        #expect(tracked?.worktree.id == worktree.id)
    }

    @Test("remote lexical and physical symlink identities match and forget tracked state")
    func remoteSymlinkIdentitiesMatchAndForgetTrackedWorktree() async throws {
        let project = Project(name: "Repo", path: "~/repo")
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let worktree = Worktree(name: "Feature", path: "~/repo-feature-alias", isPrimary: false)
        let external = Worktree(
            name: "Feature external",
            path: "/srv/repos/repo-feature",
            source: .external,
            isPrimary: false
        )
        let context = makeContext(project: project, worktrees: [primary, worktree, external])
        let remote = WorkspaceContext.ssh(SSHDestination(host: "example.com"))
        let physicalPath = "/srv/repos/repo-feature"
        let primaryResolution = GitWorktreeService.WorktreePathResolution(
            path: "/home/test/repo",
            identityPaths: ["/home/test/repo"],
            remoteHomePath: "/home/test"
        )
        let storedResolution = GitWorktreeService.WorktreePathResolution(
            path: physicalPath,
            identityPaths: ["/home/test/repo-feature-alias", physicalPath],
            remoteHomePath: "/home/test"
        )
        let externalResolution = GitWorktreeService.WorktreePathResolution(
            path: physicalPath,
            identityPaths: [physicalPath],
            remoteHomePath: "/home/test"
        )

        let tracked = try await MuxyAPI.Git.trackedWorktree(
            path: physicalPath,
            identityPaths: [physicalPath],
            project: project,
            context: context,
            workspaceContext: remote,
            remoteHomePath: "/home/test",
            deadline: OperationDeadline(timeout: 10),
            remotePathResolver: { _, _, _, _, _ in
                [primaryResolution, storedResolution, externalResolution]
            }
        )
        #expect(tracked?.worktree.id == worktree.id)
        #expect(Set(tracked?.matchingWorktrees.map(\.id) ?? []) == Set([worktree.id, external.id]))
        if let tracked {
            MuxyAPI.Git.forgetWorktrees(
                project: tracked.project,
                worktrees: tracked.matchingWorktrees,
                context: context
            )
        }

        #expect(!context.worktreeStore.list(for: project.id).contains { $0.id == worktree.id })
        #expect(!context.worktreeStore.list(for: project.id).contains { $0.id == external.id })
    }

    @Test("tracked lookup retries when remote resolution changes stored identities")
    func trackedLookupRetriesAfterStoreMutation() async throws {
        let project = Project(name: "Repo", path: "~/repo")
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let managed = Worktree(name: "Feature", path: "~/repo-feature-alias", isPrimary: false)
        let external = Worktree(
            name: "Feature external",
            path: "/srv/repos/repo-feature",
            source: .external,
            isPrimary: false
        )
        let context = makeContext(project: project, worktrees: [primary, managed])
        let physicalPath = external.path
        let resolver = MutatingRemotePathResolver(
            resolutionsByPath: [
                primary.path: GitWorktreeService.WorktreePathResolution(
                    path: "/home/test/repo",
                    identityPaths: ["/home/test/repo"],
                    remoteHomePath: "/home/test"
                ),
                managed.path: GitWorktreeService.WorktreePathResolution(
                    path: physicalPath,
                    identityPaths: ["/home/test/repo-feature-alias", physicalPath],
                    remoteHomePath: "/home/test"
                ),
                external.path: GitWorktreeService.WorktreePathResolution(
                    path: physicalPath,
                    identityPaths: [physicalPath],
                    remoteHomePath: "/home/test"
                ),
            ],
            firstMutation: {
                context.worktreeStore.restoreProjectWorktrees(
                    [primary, managed, external],
                    for: project.id
                )
            }
        )

        let tracked = try await MuxyAPI.Git.trackedWorktree(
            path: physicalPath,
            identityPaths: [physicalPath],
            project: project,
            context: context,
            workspaceContext: .ssh(SSHDestination(host: "example.com")),
            remoteHomePath: "/home/test",
            deadline: OperationDeadline(timeout: 10),
            remotePathResolver: { paths, _, _, _, _ in
                try await resolver.resolve(paths)
            }
        )

        #expect(resolver.invocationCount == 2)
        #expect(tracked?.worktree.id == managed.id)
        #expect(Set(tracked?.matchingWorktrees.map(\.id) ?? []) == Set([managed.id, external.id]))
    }

    @Test("a symlinked extension path resolves, matches, and forgets tracked state")
    func symlinkedPathMatchesAndForgetsTrackedWorktree() async throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-api-symlink-\(UUID().uuidString)", isDirectory: true)
        let repoPath = root.appendingPathComponent("repo", isDirectory: true)
        let worktreePath = root.appendingPathComponent("feature", isDirectory: true)
        let aliasPath = root.appendingPathComponent("feature-alias")
        try FileManager.default.createDirectory(at: repoPath, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: worktreePath, withIntermediateDirectories: true)
        try FileManager.default.createSymbolicLink(at: aliasPath, withDestinationURL: worktreePath)
        defer { try? FileManager.default.removeItem(at: root) }
        let project = Project(name: "Repo", path: repoPath.path)
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let worktree = Worktree(name: "Feature", path: worktreePath.path, isPrimary: false)
        let context = makeContext(project: project, worktrees: [primary, worktree])
        let resolution = try await GitWorktreeService.resolveWorktreePath(
            aliasPath.path,
            repoPath: project.path,
            context: .local,
            timeout: 10
        )

        let tracked = try await MuxyAPI.Git.trackedWorktree(
            path: resolution.path,
            identityPaths: resolution.identityPaths,
            project: project,
            context: context
        )
        #expect(tracked?.worktree.id == worktree.id)
        if let tracked {
            MuxyAPI.Git.forgetWorktree(project: tracked.project, worktree: tracked.worktree, context: context)
        }

        #expect(!context.worktreeStore.list(for: project.id).contains { $0.id == worktree.id })
    }

    @Test("default-force retry forgets a deregistered residual without teardown")
    func defaultForceRetryPreservesDeregisteredResidualWithoutTeardown() async throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-api-residual-retry-\(UUID().uuidString)", isDirectory: true)
        let repoPath = root.appendingPathComponent("repo", isDirectory: true)
        let residualPath = root.appendingPathComponent("reused-directory", isDirectory: true)
        try FileManager.default.createDirectory(at: repoPath, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: residualPath, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        try await GitRepositoryService().initRepository(repoPath: repoPath.path)
        let configDirectory = repoPath.appendingPathComponent(".muxy", isDirectory: true)
        try FileManager.default.createDirectory(at: configDirectory, withIntermediateDirectories: true)
        let config = WorktreeConfig(
            setup: [],
            teardown: [WorktreeConfig.SetupCommand(command: "touch teardown-ran")]
        )
        try JSONEncoder().encode(config).write(to: configDirectory.appendingPathComponent("worktree.json"))

        let project = Project(name: "Repo", path: repoPath.path)
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let worktree = Worktree(name: "Feature", path: residualPath.path, source: .muxy, isPrimary: false)
        let extensionID = "residual-retry-\(UUID().uuidString)"
        let context = makeContext(
            project: project,
            worktrees: [primary, worktree],
            extensionID: extensionID,
            consent: gitWriteConsent(
                extensionID: extensionID,
                fileURL: root.appendingPathComponent("grants.json")
            )
        )

        let result = await MuxyAPI.Git.removeWorktree(
            projectIdentifier: project.id.uuidString,
            path: residualPath.path,
            force: false,
            timeoutMs: 10000,
            context: context
        )
        let resolvedResidualPath = try await GitWorktreeService.canonicalLocalPaths(
            [residualPath.path],
            deadline: OperationDeadline(timeout: 10)
        )[0]

        #expect(result == .success(MuxyAPI.Git.RemoveWorktreeResult(path: resolvedResidualPath, dirRemoved: false)))
        #expect(FileManager.default.fileExists(atPath: residualPath.path))
        #expect(!FileManager.default.fileExists(atPath: residualPath.appendingPathComponent("teardown-ran").path))
        #expect(!context.worktreeStore.list(for: project.id).contains { $0.id == worktree.id })
    }

    @Test("default-force retry resolves a dangling alias and forgets a gone worktree")
    func defaultForceRetryResolvesDanglingAliasForGoneWorktree() async throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-api-dangling-retry-\(UUID().uuidString)", isDirectory: true)
        let repoPath = root.appendingPathComponent("repo", isDirectory: true)
        let missingPath = root.appendingPathComponent("removed-worktree", isDirectory: true)
        let aliasPath = root.appendingPathComponent("worktree-alias")
        try FileManager.default.createDirectory(at: repoPath, withIntermediateDirectories: true)
        try FileManager.default.createSymbolicLink(
            atPath: aliasPath.path,
            withDestinationPath: missingPath.path
        )
        defer { try? FileManager.default.removeItem(at: root) }
        try await GitRepositoryService().initRepository(repoPath: repoPath.path)

        let project = Project(name: "Repo", path: repoPath.path)
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let worktree = Worktree(name: "Feature", path: missingPath.path, source: .muxy, isPrimary: false)
        let extensionID = "dangling-retry-\(UUID().uuidString)"
        let context = makeContext(
            project: project,
            worktrees: [primary, worktree],
            extensionID: extensionID,
            consent: gitWriteConsent(
                extensionID: extensionID,
                fileURL: root.appendingPathComponent("grants.json")
            )
        )

        let result = await MuxyAPI.Git.removeWorktree(
            projectIdentifier: project.id.uuidString,
            path: aliasPath.path,
            force: false,
            timeoutMs: 10000,
            context: context
        )
        let resolvedMissingPath = try await GitWorktreeService.canonicalLocalPaths(
            [missingPath.path],
            deadline: OperationDeadline(timeout: 10)
        )[0]

        #expect(result == .success(MuxyAPI.Git.RemoveWorktreeResult(path: resolvedMissingPath, dirRemoved: true)))
        #expect((try? FileManager.default.destinationOfSymbolicLink(atPath: aliasPath.path)) == missingPath.path)
        #expect(!context.worktreeStore.list(for: project.id).contains { $0.id == worktree.id })
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

    @Test("tracked worktree lookup rejects an expired deadline")
    func trackedWorktreeLookupHonorsDeadline() async {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let worktree = Worktree(name: "Feature", path: "/tmp/repo-feature", isPrimary: false)
        let context = makeContext(project: project, worktrees: [worktree])

        await #expect(throws: AsyncTimeoutError.self) {
            try await MuxyAPI.Git.trackedWorktree(
                path: worktree.path,
                project: project,
                context: context,
                deadline: OperationDeadline(timeout: 0)
            )
        }
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

    private func makeContext(
        project: Project,
        worktrees: [Worktree],
        extensionID: String = "test",
        consent: ExtensionConsentService? = nil
    ) -> MuxyAPI.Git.Context {
        makeContext(
            projects: [project],
            worktrees: [project.id: worktrees],
            extensionID: extensionID,
            consent: consent
        )
    }

    private func makeContext(
        projects: [Project],
        worktrees: [UUID: [Worktree]],
        extensionID: String = "test",
        consent: ExtensionConsentService? = nil
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
            extensionID: extensionID,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore,
            consent: consent ?? .shared
        )
    }

    private func gitWriteConsent(extensionID: String, fileURL: URL) -> ExtensionConsentService {
        let grantStore = ExtensionGrantStore(fileURL: fileURL)
        grantStore.add(ExtensionGrantRule(
            extensionID: extensionID,
            verb: .gitWrite,
            match: .gitOperationEquals("worktree.remove"),
            decision: .allow
        ))
        return ExtensionConsentService(grantStore: grantStore)
    }
}

private final class MutatingRemotePathResolver: @unchecked Sendable {
    private let lock = NSLock()
    private var invocations = 0
    private let resolutionsByPath: [String: GitWorktreeService.WorktreePathResolution]
    private let firstMutation: @MainActor @Sendable () -> Void

    init(
        resolutionsByPath: [String: GitWorktreeService.WorktreePathResolution],
        firstMutation: @escaping @MainActor @Sendable () -> Void
    ) {
        self.resolutionsByPath = resolutionsByPath
        self.firstMutation = firstMutation
    }

    var invocationCount: Int {
        lock.withLock { invocations }
    }

    func resolve(_ paths: [String]) async throws -> [GitWorktreeService.WorktreePathResolution] {
        let invocation = lock.withLock {
            invocations += 1
            return invocations
        }
        if invocation == 1 {
            await firstMutation()
        }
        return try paths.map { path in
            guard let resolution = resolutionsByPath[path] else {
                throw GitWorktreeService.GitWorktreeError.commandFailed(
                    "Missing test resolution for \(path)."
                )
            }
            return resolution
        }
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
