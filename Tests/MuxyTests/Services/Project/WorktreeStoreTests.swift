import Foundation
import MuxyShared
import Testing

@testable import Muxy

@Suite("WorktreeStore")
@MainActor
struct WorktreeStoreTests {
    @Test("ensuring a primary worktree notifies observers")
    func ensurePrimaryNotifiesObservers() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let store = WorktreeStore(persistence: WorktreePersistenceStub(initial: [:]))
        var change: (projectID: UUID, worktreeID: UUID?)?
        store.onWorktreesChanged = { change = ($0, $1) }

        store.ensurePrimary(for: project)

        #expect(change?.projectID == project.id)
        #expect(change?.worktreeID == store.primary(for: project.id)?.id)
    }

    @Test("worktree activity persists")
    func worktreeActivityPersists() throws {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let worktree = Worktree(name: project.name, path: project.path, isPrimary: true)
        let persistence = WorktreePersistenceStub(initial: [project.id: [worktree]])
        let store = WorktreeStore(persistence: persistence, projects: [project])

        store.markActive(projectID: project.id, worktreeID: worktree.id)

        let lastActiveAt = try #require(store.worktree(projectID: project.id, worktreeID: worktree.id)?.lastActiveAt)
        let reloaded = WorktreeStore(persistence: persistence, projects: [project])
        #expect(reloaded.worktree(projectID: project.id, worktreeID: worktree.id)?.lastActiveAt == lastActiveAt)
    }

    @Test("loading repairs duplicate worktree identifiers without dropping records")
    func loadingRepairsDuplicateWorktreeIDs() throws {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let duplicateID = UUID()
        let persistence = WorktreePersistenceStub(initial: [
            project.id: [
                Worktree(name: project.name, path: project.path, isPrimary: true),
                Worktree(id: duplicateID, name: "Feature A", path: "/tmp/repo-a", isPrimary: false),
                Worktree(id: duplicateID, name: "Feature B", path: "/tmp/repo-b", isPrimary: false),
            ],
        ])

        let store = WorktreeStore(persistence: persistence, projects: [project])
        let loaded = store.list(for: project.id)

        #expect(loaded.count == 3)
        #expect(Set(loaded.map(\.id)).count == 3)
        #expect(loaded.contains { $0.id == duplicateID && $0.path == "/tmp/repo-a" })
        #expect(loaded.contains { $0.id != duplicateID && $0.path == "/tmp/repo-b" })
        #expect(Set(try persistence.loadWorktrees(projectID: project.id).map(\.id)).count == 3)
    }

    @Test("local path ownership wins over an identical remote path")
    func localPathOwnershipWinsRemoteCollision() {
        let sharedPath = "/workspace/api"
        let local = Project(name: "Local", path: sharedPath)
        let remote = RemoteProject(name: "Remote", path: sharedPath).asProject(
            workspaceID: UUID(),
            sortOrder: 0
        )
        let store = WorktreeStore(persistence: WorktreePersistenceStub(initial: [:]))

        store.loadAll(projects: [local, remote])

        #expect(store.projectID(forWorktreePath: sharedPath) == local.id)
    }

    @Test("project removal only blocks removal preparation for that project")
    func projectRemovalOnlyBlocksOwnWorktreePreparation() async {
        let sharedPath = "/workspace/feature"
        let localProjectID = UUID()
        let remoteProjectID = UUID()
        let localWorktree = Worktree(name: "Local", path: sharedPath, isPrimary: false)
        let remoteWorktree = Worktree(name: "Remote", path: sharedPath, isPrimary: false)
        let store = WorktreeStore(persistence: WorktreePersistenceStub(initial: [:]))
        store.add(localWorktree, to: localProjectID)
        store.add(remoteWorktree, to: remoteProjectID, context: .ssh(SSHDestination(host: "example.com")))

        #expect(await store.beginProjectRemoval(remoteProjectID))

        #expect(store.beginRemovalPreparation(worktree: localWorktree, projectID: localProjectID))
        #expect(!store.beginRemovalPreparation(worktree: remoteWorktree, projectID: remoteProjectID))
    }

    @Test("removing a project notifies worktree observers")
    func removingProjectNotifiesObservers() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let store = WorktreeStore(persistence: WorktreePersistenceStub(initial: [:]))
        store.ensurePrimary(for: project)
        var change: (projectID: UUID, worktreeID: UUID?)?
        store.onWorktreesChanged = { change = ($0, $1) }

        store.removeProject(project.id)

        #expect(change?.projectID == project.id)
        #expect(change?.worktreeID == nil)
    }

    @Test("restoring worktrees notifies observers")
    func restoringWorktreesNotifiesObservers() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let store = WorktreeStore(persistence: WorktreePersistenceStub(initial: [:]))
        let worktree = Worktree(name: "Repo", path: "/tmp/repo", isPrimary: true)
        var change: (projectID: UUID, worktreeID: UUID?)?
        store.onWorktreesChanged = { change = ($0, $1) }

        store.restoreProjectWorktrees([worktree], for: project)

        #expect(change?.projectID == project.id)
        #expect(change?.worktreeID == nil)
    }

    @Test("restoring local worktrees owns a path shared with a remote project")
    func restoringLocalWorktreesOwnsSharedRemotePath() {
        let sharedPath = "/workspace/api"
        let local = Project(name: "Local", path: sharedPath)
        let remote = RemoteProject(name: "Remote", path: sharedPath).asProject(
            workspaceID: UUID(),
            sortOrder: 0
        )
        let store = WorktreeStore(persistence: WorktreePersistenceStub(initial: [:]))

        store.restoreProjectWorktrees(
            [Worktree(name: remote.name, path: sharedPath, isPrimary: true)],
            for: remote
        )
        store.restoreProjectWorktrees(
            [Worktree(name: local.name, path: sharedPath, isPrimary: true)],
            for: local
        )

        #expect(store.projectID(forWorktreePath: sharedPath) == local.id)
    }

    @Test("Worktree decodes legacy records without source metadata")
    func worktreeLegacyDecodeDefaultsToMuxy() throws {
        let json = """
        {
          "id": "\(UUID().uuidString)",
          "name": "feature-a",
          "path": "/tmp/feature-a",
          "branch": "feature-a",
          "ownsBranch": false,
          "isPrimary": false,
          "createdAt": "2024-01-01T00:00:00Z"
        }
        """
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let worktree = try decoder.decode(Worktree.self, from: Data(json.utf8))

        #expect(worktree.source == .muxy)
        #expect(worktree.isExternallyManaged == false)
    }

    @Test("WorktreeDTO decodes legacy payloads without removal metadata")
    func worktreeDTOLegacyDecodeDefaultsRemovalCapability() throws {
        let json = """
        {
          "id": "\(UUID().uuidString)",
          "name": "feature-a",
          "path": "/tmp/feature-a",
          "branch": "feature-a",
          "isPrimary": false,
          "createdAt": "2024-01-01T00:00:00Z"
        }
        """
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let worktree = try decoder.decode(WorktreeDTO.self, from: Data(json.utf8))

        #expect(worktree.canBeRemoved)
    }

    @Test("removal preparation serializes a worktree removal lifecycle")
    func removalPreparationSerializesLifecycle() {
        let projectID = UUID()
        let store = WorktreeStore(persistence: WorktreePersistenceStub(initial: [:]))
        let worktree = Worktree(
            name: "feature",
            path: "/tmp/repo-feature",
            isPrimary: false
        )

        #expect(store.beginRemovalPreparation(worktree: worktree, projectID: projectID))
        #expect(!store.beginRemovalPreparation(worktree: worktree, projectID: projectID))
        #expect(store.hasRemovalPreparation)
        #expect(store.isPreparingRemoval(worktreeID: worktree.id))
        #expect(store.isRemovalInProgress(worktreeID: worktree.id))

        store.endRemovalPreparation(worktreeID: worktree.id)

        #expect(!store.hasRemovalPreparation)
        #expect(!store.isPreparingRemoval(worktreeID: worktree.id))
        #expect(!store.isRemovalInProgress(worktreeID: worktree.id))
    }

    @Test("removal preparation is cleared when the worktree leaves the list")
    func removalPreparationClearsWhenWorktreeDisappears() {
        let projectID = UUID()
        let store = WorktreeStore(persistence: WorktreePersistenceStub(initial: [:]))
        let worktree = Worktree(
            name: "feature",
            path: "/tmp/repo-feature",
            isPrimary: false
        )
        store.add(worktree, to: projectID)

        #expect(store.beginRemovalPreparation(worktree: worktree, projectID: projectID))
        #expect(store.isPreparingRemoval(worktreeID: worktree.id))

        store.remove(worktreeID: worktree.id, from: projectID)

        #expect(!store.isPreparingRemoval(worktreeID: worktree.id))
        #expect(!store.hasRemovalPreparation)
    }

    @Test("removal preparation is cleared when the project is removed")
    func removalPreparationClearsWhenProjectRemoved() {
        let projectID = UUID()
        let store = WorktreeStore(persistence: WorktreePersistenceStub(initial: [:]))
        let worktree = Worktree(
            name: "feature",
            path: "/tmp/repo-feature",
            isPrimary: false
        )
        store.add(worktree, to: projectID)
        _ = store.beginRemovalPreparation(worktree: worktree, projectID: projectID)

        store.removeProject(projectID)

        #expect(!store.hasRemovalPreparation)
    }

    @Test("refreshFromGit imports missing external worktrees and preserves existing IDs by path")
    func refreshFromGitImportsAndPreservesIDs() async throws {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let existingID = UUID()
        let createdAt = Date(timeIntervalSince1970: 123)
        let persistence = WorktreePersistenceStub(
            initial: [
                project.id: [
                    Worktree(
                        name: project.name,
                        path: project.path,
                        branch: "main",
                        isPrimary: true
                    ),
                    Worktree(
                        id: existingID,
                        name: "Feature A",
                        path: "/tmp/repo-feature-a",
                        branch: "feature-a-old",
                        source: .muxy,
                        isPrimary: false,
                        createdAt: createdAt
                    ),
                ]
            ]
        )
        let gitService = GitWorktreeListingStub(recordsByRepoPath: [
            project.path: [
                GitWorktreeRecord(
                    path: project.path,
                    branch: "main",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
                GitWorktreeRecord(
                    path: "/tmp/repo-feature-a",
                    branch: "feature-a",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
                GitWorktreeRecord(
                    path: "/tmp/repo-feature-b",
                    branch: "feature-b",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
            ]
        ])
        let store = WorktreeStore(
            persistence: persistence,
            listGitWorktrees: gitService.listWorktrees,
            projects: [project]
        )

        let worktrees = try await store.refreshFromGit(project: project)

        #expect(worktrees.count == 3)
        #expect(worktrees[0].isPrimary)

        let preserved = try #require(worktrees.first(where: { $0.path == "/tmp/repo-feature-a" }))
        #expect(preserved.id == existingID)
        #expect(preserved.branch == "feature-a")
        #expect(preserved.source == .muxy)
        #expect(preserved.createdAt == createdAt)

        let imported = try #require(worktrees.first(where: { $0.path == "/tmp/repo-feature-b" }))
        #expect(imported.name == "feature-b")
        #expect(imported.branch == "feature-b")
        #expect(imported.source == .external)
        #expect(imported.isExternallyManaged)
    }

    @Test("project removal waits for an active worktree creation and freezes later mutations")
    func projectRemovalWaitsForActiveCreation() async throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-worktree-removal-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let project = Project(name: "Repo", path: root.appendingPathComponent("repo").path)
        let gate = WorktreeMutationTestGate()
        let store = WorktreeStore(
            persistence: WorktreePersistenceStub(initial: [:]),
            addGitWorktree: { _, _, _, _, _ in await gate.enterAndWait() }
        )
        let request = WorktreeCreationRequest(
            name: "Feature",
            path: root.appendingPathComponent("feature").path,
            branch: "feature",
            createBranch: true,
            baseBranch: nil,
            runSetup: false
        )

        let creation = Task { @MainActor in
            try await store.createWorktree(project: project, request: request)
        }
        await gate.waitUntilEntered()
        let removal = Task { @MainActor in
            await store.beginProjectRemoval(project.id)
        }
        for _ in 0 ..< 10 where !store.isProjectRemovalInProgress(project.id) {
            await Task.yield()
        }
        #expect(store.isProjectRemovalInProgress(project.id))

        await #expect(throws: WorktreeMutationError.self) {
            try await store.createWorktree(project: project, request: request)
        }
        let lateWorktree = Worktree(
            name: "Late",
            path: root.appendingPathComponent("late").path,
            branch: "late",
            isPrimary: false
        )
        store.add(lateWorktree, to: project.id)
        #expect(!store.list(for: project.id).contains(lateWorktree))

        await gate.release()
        let created = try await creation.value
        #expect(await removal.value)
        #expect(store.list(for: project.id).contains(created))
        store.cancelProjectRemoval(project.id)
    }

    @Test("worktree creation retries identity resolution after concurrent changes")
    func creationRetriesIdentityResolutionAfterConcurrentChanges() async throws {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let requestedPath = "../repo-feature"
        let resolvedPath = "/tmp/repo-feature"
        let aliasCreatedAt = Date(timeIntervalSince1970: 1_700_000_000)
        let aliasLastActiveAt = Date(timeIntervalSince1970: 1_700_000_100)
        let existingAlias = Worktree(
            name: "feature",
            path: resolvedPath,
            branch: "feature",
            source: .external,
            isPrimary: false,
            createdAt: aliasCreatedAt,
            lastActiveAt: aliasLastActiveAt
        )
        let gate = FirstPathResolutionGate()
        let store = WorktreeStore(
            persistence: WorktreePersistenceStub(initial: [:]),
            addGitWorktree: { _, _, _, _, _ in },
            pathResolver: GatedWorkspacePathResolver(
                gate: gate,
                resolvedPaths: [requestedPath: resolvedPath]
            ),
            projects: [project]
        )
        let request = WorktreeCreationRequest(
            name: "feature",
            path: requestedPath,
            branch: "feature",
            createBranch: true,
            baseBranch: nil
        )

        let creation = Task { @MainActor in
            try await store.createWorktree(project: project, request: request)
        }
        await gate.waitUntilEntered()
        store.add(existingAlias, to: project.id)
        await gate.release()
        let created = try await creation.value

        let secondary = store.list(for: project.id).filter { !$0.isPrimary }
        #expect(secondary.count == 1)
        #expect(created.id == existingAlias.id)
        #expect(created.createdAt == aliasCreatedAt)
        #expect(created.lastActiveAt == aliasLastActiveAt)
        #expect(created.source == .muxy)
        #expect(secondary.first?.id == created.id)
        #expect(await gate.callCount() == 2)
    }

    @Test("refreshFromGit preserves activity changes during path resolution")
    func refreshFromGitPreservesConcurrentActivity() async throws {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let feature = Worktree(
            name: "feature",
            path: "/tmp/repo-feature",
            branch: "feature",
            isPrimary: false
        )
        let persistence = WorktreePersistenceStub(initial: [
            project.id: [
                Worktree(name: project.name, path: project.path, branch: "main", isPrimary: true),
                feature,
            ],
        ])
        let gitService = GitWorktreeListingStub(recordsByRepoPath: [
            project.path: [
                GitWorktreeRecord(
                    path: project.path,
                    branch: "main",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
                GitWorktreeRecord(
                    path: feature.path,
                    branch: feature.branch,
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
            ],
        ])
        let gate = FirstPathResolutionGate()
        let store = WorktreeStore(
            persistence: persistence,
            listGitWorktrees: gitService.listWorktrees,
            pathResolver: GatedWorkspacePathResolver(gate: gate),
            projects: [project]
        )

        let refresh = Task { @MainActor in
            try await store.refreshFromGit(project: project)
        }
        await gate.waitUntilEntered()
        store.markActive(projectID: project.id, worktreeID: feature.id)
        await gate.release()
        let refreshed = try await refresh.value

        let refreshedFeature = try #require(refreshed.first(where: { $0.id == feature.id }))
        #expect(refreshedFeature.lastActiveAt != nil)
        #expect(store.worktree(projectID: project.id, worktreeID: feature.id)?.lastActiveAt == refreshedFeature.lastActiveAt)
        #expect(await gate.callCount() == 1)
    }

    @Test("local worktree creation runs setup after storing the worktree")
    func localCreationRunsSetup() async throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-worktree-setup-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let project = Project(name: "Repo", path: root.appendingPathComponent("repo").path)
        let setupCapture = WorktreeSetupCapture()
        let approval = WorktreeConfig.ProjectHookApproval(resolvedCommands: [])
        let store = WorktreeStore(
            persistence: WorktreePersistenceStub(initial: [:]),
            addGitWorktree: { _, _, _, _, _ in },
            runWorktreeSetup: { setupCapture.record(projectPath: $0, worktree: $1, approval: $2) }
        )
        let request = WorktreeCreationRequest(
            name: "Feature",
            path: root.appendingPathComponent("feature").path,
            branch: "feature",
            createBranch: true,
            baseBranch: nil,
            runSetup: true,
            projectHookApproval: approval
        )

        let worktree = try await store.createWorktree(project: project, request: request)

        #expect(setupCapture.projectPaths == [project.path])
        #expect(setupCapture.worktreeIDs == [worktree.id])
        #expect(setupCapture.approvals == [approval])
        #expect(store.list(for: project.id).contains(worktree))
    }

    @Test("worktree creation schedules reconciliation after path resolution fails")
    func creationReconcilesAfterPathResolutionFailure() async throws {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let requestedPath = "../repo-feature"
        let resolvedPath = "/tmp/repo-feature"
        let external = Worktree(
            name: "feature",
            path: resolvedPath,
            branch: "feature",
            source: .external,
            isPrimary: false
        )
        let persistence = WorktreePersistenceStub(initial: [
            project.id: [
                Worktree(name: project.name, path: project.path, branch: "main", isPrimary: true),
                external,
            ],
        ])
        let gitService = GitWorktreeListingStub(recordsByRepoPath: [
            project.path: [
                GitWorktreeRecord(
                    path: project.path,
                    branch: "main",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
                GitWorktreeRecord(
                    path: resolvedPath,
                    branch: "feature",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
            ],
        ])
        let resolver = RecoveringWorkspacePathResolver(resolvedPaths: [requestedPath: resolvedPath])
        let store = WorktreeStore(
            persistence: persistence,
            listGitWorktrees: gitService.listWorktrees,
            addGitWorktree: { _, _, _, _, _ in },
            pathResolver: resolver,
            projects: [project]
        )
        let request = WorktreeCreationRequest(
            name: "feature",
            path: requestedPath,
            branch: "feature",
            createBranch: true,
            baseBranch: nil
        )

        let created = try await store.createWorktree(project: project, request: request)
        await resolver.waitUntilRecovered()
        for _ in 0 ..< 100 where store.list(for: project.id).filter({ !$0.isPrimary }).count != 1 {
            try await Task.sleep(for: .milliseconds(10))
        }

        let secondary = store.list(for: project.id).filter { !$0.isPrimary }
        #expect(secondary.count == 1)
        #expect(secondary.first?.id == created.id)
        #expect(secondary.first?.source == .muxy)
        #expect(secondary.first?.path == requestedPath)
    }

    @Test("local worktree creation defaults setup to off")
    func localCreationDefaultsSetupOff() async throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-worktree-setup-opt-out-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let project = Project(name: "Repo", path: root.appendingPathComponent("repo").path)
        let setupCapture = WorktreeSetupCapture()
        let store = WorktreeStore(
            persistence: WorktreePersistenceStub(initial: [:]),
            addGitWorktree: { _, _, _, _, _ in },
            runWorktreeSetup: { setupCapture.record(projectPath: $0, worktree: $1, approval: $2) }
        )
        let request = WorktreeCreationRequest(
            name: "Feature",
            path: root.appendingPathComponent("feature").path,
            branch: "feature",
            createBranch: true,
            baseBranch: nil
        )

        _ = try await store.createWorktree(project: project, request: request)

        #expect(setupCapture.worktreeIDs.isEmpty)
    }

    @Test("refreshFromGit re-syncs branch-derived name on branch rename but keeps custom names")
    func refreshFromGitSyncsBranchDerivedName() async throws {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let persistence = WorktreePersistenceStub(
            initial: [
                project.id: [
                    Worktree(
                        name: project.name,
                        path: project.path,
                        branch: "main",
                        isPrimary: true
                    ),
                    Worktree(
                        name: "passion729/regulus",
                        path: "/tmp/repo-wt1",
                        branch: "passion729/regulus",
                        source: .external,
                        isPrimary: false
                    ),
                    Worktree(
                        name: "My Feature",
                        path: "/tmp/repo-wt2",
                        branch: "feature-old",
                        source: .external,
                        isPrimary: false
                    ),
                ]
            ]
        )
        let gitService = GitWorktreeListingStub(recordsByRepoPath: [
            project.path: [
                GitWorktreeRecord(
                    path: project.path,
                    branch: "main",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
                GitWorktreeRecord(
                    path: "/tmp/repo-wt1",
                    branch: "passion729/greeting",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
                GitWorktreeRecord(
                    path: "/tmp/repo-wt2",
                    branch: "feature-new",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
            ]
        ])
        let store = WorktreeStore(
            persistence: persistence,
            listGitWorktrees: gitService.listWorktrees,
            projects: [project]
        )

        let worktrees = try await store.refreshFromGit(project: project)

        let tracked = try #require(worktrees.first(where: { $0.path == "/tmp/repo-wt1" }))
        #expect(tracked.branch == "passion729/greeting")
        #expect(tracked.name == "passion729/greeting")

        let custom = try #require(worktrees.first(where: { $0.path == "/tmp/repo-wt2" }))
        #expect(custom.branch == "feature-new")
        #expect(custom.name == "My Feature")
    }

    @Test("refreshFromGit keeps a branch-derived name when the worktree goes detached")
    func refreshFromGitKeepsNameOnDetachedHead() async throws {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let persistence = WorktreePersistenceStub(
            initial: [
                project.id: [
                    Worktree(
                        name: project.name,
                        path: project.path,
                        branch: "main",
                        isPrimary: true
                    ),
                    Worktree(
                        name: "passion729/regulus",
                        path: "/tmp/repo-wt1",
                        branch: "passion729/regulus",
                        source: .external,
                        isPrimary: false
                    ),
                ]
            ]
        )
        let gitService = GitWorktreeListingStub(recordsByRepoPath: [
            project.path: [
                GitWorktreeRecord(
                    path: project.path,
                    branch: "main",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
                GitWorktreeRecord(
                    path: "/tmp/repo-wt1",
                    branch: nil,
                    head: "abc123",
                    isBare: false,
                    isDetached: true
                ),
            ]
        ])
        let store = WorktreeStore(
            persistence: persistence,
            listGitWorktrees: gitService.listWorktrees,
            projects: [project]
        )

        let worktrees = try await store.refreshFromGit(project: project)

        let detached = try #require(worktrees.first(where: { $0.path == "/tmp/repo-wt1" }))
        #expect(detached.branch == nil)
        #expect(detached.name == "passion729/regulus")
    }

    @Test("refreshFromGit keeps missing Muxy-managed worktrees")
    func refreshFromGitKeepsMissingMuxyManagedEntries() async throws {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let persistence = WorktreePersistenceStub(
            initial: [
                project.id: [
                    Worktree(name: project.name, path: project.path, isPrimary: true),
                    Worktree(
                        name: "Retained",
                        path: "/tmp/repo-retained",
                        branch: "retained",
                        source: .muxy,
                        isPrimary: false
                    ),
                ]
            ]
        )
        let gitService = GitWorktreeListingStub(recordsByRepoPath: [
            project.path: [
                GitWorktreeRecord(
                    path: project.path,
                    branch: "main",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
            ]
        ])
        let store = WorktreeStore(
            persistence: persistence,
            listGitWorktrees: gitService.listWorktrees,
            projects: [project]
        )

        let worktrees = try await store.refreshFromGit(project: project)

        #expect(worktrees.count == 2)
        #expect(worktrees.contains(where: { $0.path == "/tmp/repo-retained" }))
    }

    @Test("refreshFromGit removes missing external worktrees")
    func refreshFromGitRemovesMissingExternalEntries() async throws {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let persistence = WorktreePersistenceStub(
            initial: [
                project.id: [
                    Worktree(name: project.name, path: project.path, isPrimary: true),
                    Worktree(
                        name: "External",
                        path: "/tmp/repo-external",
                        branch: "external",
                        source: .external,
                        isPrimary: false
                    ),
                ]
            ]
        )
        let gitService = GitWorktreeListingStub(recordsByRepoPath: [
            project.path: [
                GitWorktreeRecord(
                    path: project.path,
                    branch: "main",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
            ]
        ])
        let store = WorktreeStore(
            persistence: persistence,
            listGitWorktrees: gitService.listWorktrees,
            projects: [project]
        )

        let worktrees = try await store.refreshFromGit(project: project)

        #expect(worktrees.count == 1)
        #expect(worktrees.allSatisfy { !$0.isExternallyManaged })
        #expect(!worktrees.contains(where: { $0.path == "/tmp/repo-external" }))
    }

    @Test("refreshFromGit ignores bare and prunable records")
    func refreshFromGitIgnoresUnusableRecords() async throws {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let persistence = WorktreePersistenceStub(
            initial: [
                project.id: [
                    Worktree(name: project.name, path: project.path, isPrimary: true),
                ]
            ]
        )
        let gitService = GitWorktreeListingStub(recordsByRepoPath: [
            project.path: [
                GitWorktreeRecord(
                    path: project.path,
                    branch: "main",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
                GitWorktreeRecord(
                    path: "/tmp/repo-bare",
                    branch: nil,
                    head: nil,
                    isBare: true,
                    isDetached: false
                ),
                GitWorktreeRecord(
                    path: "/tmp/repo-prunable",
                    branch: "feature-prunable",
                    head: nil,
                    isBare: false,
                    isDetached: false,
                    isPrunable: true
                ),
                GitWorktreeRecord(
                    path: "/tmp/repo-live",
                    branch: "feature-live",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
            ]
        ])
        let store = WorktreeStore(
            persistence: persistence,
            listGitWorktrees: gitService.listWorktrees,
            projects: [project]
        )

        let worktrees = try await store.refreshFromGit(project: project)

        #expect(worktrees.count == 2)
        #expect(worktrees.contains(where: { $0.path == "/tmp/repo-live" }))
        #expect(!worktrees.contains(where: { $0.path == "/tmp/repo-bare" }))
        #expect(!worktrees.contains(where: { $0.path == "/tmp/repo-prunable" }))
    }

    @Test("refreshFromGit collapses duplicate persisted paths into one entry")
    func refreshFromGitCollapsesDuplicatePaths() async throws {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let duplicatePath = "/tmp/repo-dupe"
        let persistence = WorktreePersistenceStub(
            initial: [
                project.id: [
                    Worktree(name: project.name, path: project.path, isPrimary: true),
                    Worktree(
                        name: "first",
                        path: duplicatePath,
                        branch: "first",
                        source: .muxy,
                        isPrimary: false
                    ),
                    Worktree(
                        name: "second",
                        path: duplicatePath,
                        branch: "second",
                        source: .muxy,
                        isPrimary: false
                    ),
                ]
            ]
        )
        let gitService = GitWorktreeListingStub(recordsByRepoPath: [
            project.path: [
                GitWorktreeRecord(
                    path: project.path,
                    branch: "main",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
                GitWorktreeRecord(
                    path: duplicatePath,
                    branch: "updated",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
            ]
        ])
        let store = WorktreeStore(
            persistence: persistence,
            listGitWorktrees: gitService.listWorktrees,
            projects: [project]
        )

        let worktrees = try await store.refreshFromGit(project: project)

        let atDuplicatePath = worktrees.filter { $0.path == duplicatePath }
        #expect(atDuplicatePath.count == 1)
        #expect(atDuplicatePath.first?.branch == "updated")
    }

    @Test("refreshFromGit collapses a Muxy and external entry sharing a path, keeping Muxy")
    func refreshFromGitCollapsesRaceDuplicateKeepingMuxy() async throws {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let sharedPath = "/tmp/repo-feature"
        let muxyID = UUID()
        let persistence = WorktreePersistenceStub(
            initial: [
                project.id: [
                    Worktree(name: project.name, path: project.path, isPrimary: true),
                    Worktree(
                        name: "imported",
                        path: sharedPath,
                        branch: "feature",
                        source: .external,
                        isPrimary: false
                    ),
                    Worktree(
                        id: muxyID,
                        name: "feature",
                        path: sharedPath,
                        branch: "feature",
                        source: .muxy,
                        isPrimary: false
                    ),
                ]
            ]
        )
        let gitService = GitWorktreeListingStub(recordsByRepoPath: [
            project.path: [
                GitWorktreeRecord(
                    path: project.path,
                    branch: "main",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
                GitWorktreeRecord(
                    path: sharedPath,
                    branch: "feature",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
            ]
        ])
        let store = WorktreeStore(
            persistence: persistence,
            listGitWorktrees: gitService.listWorktrees,
            projects: [project]
        )

        let worktrees = try await store.refreshFromGit(project: project)

        let atSharedPath = worktrees.filter { $0.path == sharedPath }
        #expect(atSharedPath.count == 1)
        #expect(atSharedPath.first?.source == .muxy)
        #expect(atSharedPath.first?.id == muxyID)
    }

    @Test("refreshFromGit reconciles alternate representations of the same path")
    func refreshFromGitReconcilesResolvedPath() async throws {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let storedPath = "~/repo-feature"
        let resolvedPath = "/home/test/repo-feature"
        let muxyID = UUID()
        let persistence = WorktreePersistenceStub(
            initial: [
                project.id: [
                    Worktree(name: project.name, path: project.path, isPrimary: true),
                    Worktree(
                        name: "feature",
                        path: resolvedPath,
                        branch: "feature",
                        source: .external,
                        isPrimary: false
                    ),
                    Worktree(
                        id: muxyID,
                        name: "stale",
                        path: storedPath,
                        branch: "stale",
                        source: .muxy,
                        isPrimary: false
                    ),
                ]
            ]
        )
        let gitService = GitWorktreeListingStub(recordsByRepoPath: [
            project.path: [
                GitWorktreeRecord(
                    path: project.path,
                    branch: "main",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
                GitWorktreeRecord(
                    path: resolvedPath,
                    branch: "feature",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
            ]
        ])
        let store = WorktreeStore(
            persistence: persistence,
            listGitWorktrees: gitService.listWorktrees,
            pathResolver: WorkspacePathResolverStub(resolvedPaths: [storedPath: resolvedPath]),
            projects: [project]
        )

        let worktrees = try await store.refreshFromGit(project: project)

        let featureWorktrees = worktrees.filter { !$0.isPrimary }
        #expect(featureWorktrees.count == 1)
        #expect(featureWorktrees.first?.id == muxyID)
        #expect(featureWorktrees.first?.source == .muxy)
        #expect(featureWorktrees.first?.path == storedPath)
        #expect(featureWorktrees.first?.branch == "feature")
        #expect(featureWorktrees.first?.name == "feature")
    }

    @Test("refreshFromGit removes a secondary alias of the primary worktree")
    func refreshFromGitRemovesPrimaryAlias() async throws {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let aliasPath = "~/repo-alias"
        let persistence = WorktreePersistenceStub(
            initial: [
                project.id: [
                    Worktree(name: project.name, path: project.path, isPrimary: true),
                    Worktree(name: "alias", path: aliasPath, branch: "main", isPrimary: false),
                ]
            ]
        )
        let gitService = GitWorktreeListingStub(recordsByRepoPath: [
            project.path: [
                GitWorktreeRecord(
                    path: project.path,
                    branch: "main",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
            ]
        ])
        let store = WorktreeStore(
            persistence: persistence,
            listGitWorktrees: gitService.listWorktrees,
            pathResolver: WorkspacePathResolverStub(resolvedPaths: [aliasPath: project.path]),
            projects: [project]
        )

        let worktrees = try await store.refreshFromGit(project: project)

        #expect(worktrees.count == 1)
        #expect(worktrees.first?.isPrimary == true)
    }

    @Test("add reconciles an externally imported worktree at the same path instead of duplicating")
    func addReconcilesExistingExternalAtSamePath() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let sharedPath = "/tmp/repo-feature"
        let persistence = WorktreePersistenceStub(
            initial: [
                project.id: [
                    Worktree(name: project.name, path: project.path, isPrimary: true),
                    Worktree(
                        name: "feature",
                        path: sharedPath,
                        branch: "feature",
                        source: .external,
                        isPrimary: false
                    ),
                ]
            ]
        )
        let store = WorktreeStore(
            persistence: persistence,
            listGitWorktrees: GitWorktreeListingStub(recordsByRepoPath: [:]).listWorktrees,
            projects: [project]
        )

        let managed = Worktree(
            name: "feature",
            path: sharedPath,
            branch: "feature",
            source: .muxy,
            isPrimary: false
        )
        store.add(managed, to: project.id)

        let atSharedPath = store.list(for: project.id).filter { $0.path == sharedPath }
        #expect(atSharedPath.count == 1)
        #expect(atSharedPath.first?.source == .muxy)
    }

    @Test("refreshFromGit treats symlinked primary paths as the primary worktree")
    func refreshFromGitResolvesSymlinkedPrimaryPath() async throws {
        let tempRoot = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("muxy-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tempRoot, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempRoot) }

        let realRepo = tempRoot.appendingPathComponent("real-repo", isDirectory: true)
        try FileManager.default.createDirectory(at: realRepo, withIntermediateDirectories: true)
        let symlink = tempRoot.appendingPathComponent("linked-repo")
        try FileManager.default.createSymbolicLink(at: symlink, withDestinationURL: realRepo)

        let project = Project(name: "Repo", path: symlink.path)
        let persistence = WorktreePersistenceStub(
            initial: [
                project.id: [
                    Worktree(name: project.name, path: symlink.path, isPrimary: true),
                ]
            ]
        )
        let gitService = GitWorktreeListingStub(recordsByRepoPath: [
            project.path: [
                GitWorktreeRecord(
                    path: realRepo.path,
                    branch: "feat/worktree-refresh",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
            ]
        ])
        let store = WorktreeStore(
            persistence: persistence,
            listGitWorktrees: gitService.listWorktrees,
            projects: [project]
        )

        let worktrees = try await store.refreshFromGit(project: project)

        #expect(worktrees.count == 1)
        #expect(worktrees[0].isPrimary)
        #expect(worktrees[0].branch == "feat/worktree-refresh")
    }

    @Test("remove deletes externally managed worktrees")
    func removeDeletesExternalWorktree() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let external = Worktree(
            name: "feature-b",
            path: "/tmp/repo-feature-b",
            branch: "feature-b",
            source: .external,
            isPrimary: false
        )
        let persistence = WorktreePersistenceStub(
            initial: [
                project.id: [
                    Worktree(name: project.name, path: project.path, isPrimary: true),
                    external,
                ]
            ]
        )
        let store = WorktreeStore(
            persistence: persistence,
            listGitWorktrees: GitWorktreeListingStub(recordsByRepoPath: [:]).listWorktrees,
            projects: [project]
        )

        store.remove(worktreeID: external.id, from: project.id)

        #expect(!store.list(for: project.id).contains(external))
        #expect(external.canBeRemoved)
    }

    @Test("list returns all worktrees including those with no open tabs (regression: sidebar hiding)")
    func listReturnsAllWorktreesRegardlessOfTabs() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let featureA = Worktree(
            name: "feature-a",
            path: "/tmp/repo-feature-a",
            branch: "feature-a",
            source: .muxy,
            isPrimary: false
        )
        let featureB = Worktree(
            name: "feature-b",
            path: "/tmp/repo-feature-b",
            branch: "feature-b",
            source: .muxy,
            isPrimary: false
        )
        let store = WorktreeStore(
            persistence: WorktreePersistenceStub(
                initial: [
                    project.id: [primary, featureA, featureB]
                ]
            ),
            listGitWorktrees: GitWorktreeListingStub(recordsByRepoPath: [:]).listWorktrees,
            projects: [project]
        )

        let worktrees = store.list(for: project.id)

        #expect(worktrees.count == 3)
        #expect(worktrees.contains(primary))
        #expect(worktrees.contains(featureA))
        #expect(worktrees.contains(featureB))
        #expect(worktrees.filter(\.isPrimary).count == 1)
        #expect(worktrees.filter { !$0.isPrimary }.count == 2)
    }

    @Test("WorktreeDTO preserves removal capability")
    func worktreeDTOPreservesRemovalCapability() {
        let primary = Worktree(name: "Repo", path: "/tmp/repo", isPrimary: true)
        let external = Worktree(
            name: "feature-b",
            path: "/tmp/repo-feature-b",
            branch: "feature-b",
            source: .external,
            isPrimary: false
        )
        let managed = Worktree(
            name: "feature-c",
            path: "/tmp/repo-feature-c",
            branch: "feature-c",
            source: .muxy,
            isPrimary: false
        )

        #expect(primary.toDTO().canBeRemoved == false)
        #expect(external.toDTO().canBeRemoved)
        #expect(managed.toDTO().canBeRemoved)
    }
}

private final class WorktreePersistenceStub: WorktreePersisting {
    private var storage: [UUID: [Worktree]]

    init(initial: [UUID: [Worktree]]) {
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

private struct GitWorktreeListingStub: GitWorktreeListing {
    let recordsByRepoPath: [String: [GitWorktreeRecord]]

    func listWorktrees(repoPath: String) async throws -> [GitWorktreeRecord] {
        recordsByRepoPath[repoPath] ?? []
    }
}

private struct WorkspacePathResolverStub: WorkspacePathResolving {
    let resolvedPaths: [String: String]

    func resolve(
        paths: [String],
        relativeTo _: String,
        context _: WorkspaceContext,
        timeout _: TimeInterval
    ) async throws -> [WorkspacePathResolution] {
        paths.map {
            WorkspacePathResolution(path: resolvedPaths[$0] ?? $0)
        }
    }
}

private struct GatedWorkspacePathResolver: WorkspacePathResolving {
    let gate: FirstPathResolutionGate
    var resolvedPaths: [String: String] = [:]

    func resolve(
        paths: [String],
        relativeTo _: String,
        context _: WorkspaceContext,
        timeout _: TimeInterval
    ) async throws -> [WorkspacePathResolution] {
        await gate.pauseOnce()
        return paths.map { WorkspacePathResolution(path: resolvedPaths[$0] ?? $0) }
    }
}

private actor RecoveringWorkspacePathResolver: WorkspacePathResolving {
    let resolvedPaths: [String: String]
    private var calls = 0
    private var recoveryWaiters: [CheckedContinuation<Void, Never>] = []

    init(resolvedPaths: [String: String]) {
        self.resolvedPaths = resolvedPaths
    }

    func resolve(
        paths: [String],
        relativeTo _: String,
        context _: WorkspaceContext,
        timeout _: TimeInterval
    ) throws -> [WorkspacePathResolution] {
        calls += 1
        guard calls > 1 else {
            throw WorkspacePathResolverError.commandFailed("transient failure")
        }
        for waiter in recoveryWaiters {
            waiter.resume()
        }
        recoveryWaiters.removeAll()
        return paths.map { WorkspacePathResolution(path: resolvedPaths[$0] ?? $0) }
    }

    func waitUntilRecovered() async {
        guard calls < 2 else { return }
        await withCheckedContinuation { continuation in
            recoveryWaiters.append(continuation)
        }
    }
}

private actor FirstPathResolutionGate {
    private var entered = false
    private var shouldPause = true
    private var calls = 0
    private var entryWaiters: [CheckedContinuation<Void, Never>] = []
    private var releaseWaiter: CheckedContinuation<Void, Never>?

    func pauseOnce() async {
        calls += 1
        guard shouldPause else { return }
        shouldPause = false
        entered = true
        for waiter in entryWaiters {
            waiter.resume()
        }
        entryWaiters.removeAll()
        await withCheckedContinuation { continuation in
            releaseWaiter = continuation
        }
    }

    func waitUntilEntered() async {
        guard !entered else { return }
        await withCheckedContinuation { continuation in
            entryWaiters.append(continuation)
        }
    }

    func release() {
        releaseWaiter?.resume()
        releaseWaiter = nil
    }

    func callCount() -> Int {
        calls
    }
}

private actor WorktreeMutationTestGate {
    private var entered = false
    private var entryWaiters: [CheckedContinuation<Void, Never>] = []
    private var releaseWaiter: CheckedContinuation<Void, Never>?

    func enterAndWait() async {
        entered = true
        for waiter in entryWaiters {
            waiter.resume()
        }
        entryWaiters.removeAll()
        await withCheckedContinuation { continuation in
            releaseWaiter = continuation
        }
    }

    func waitUntilEntered() async {
        guard !entered else { return }
        await withCheckedContinuation { continuation in
            entryWaiters.append(continuation)
        }
    }

    func release() {
        releaseWaiter?.resume()
        releaseWaiter = nil
    }
}

@MainActor
private final class WorktreeSetupCapture {
    private(set) var projectPaths: [String] = []
    private(set) var worktreeIDs: [UUID] = []
    private(set) var approvals: [WorktreeConfig.ProjectHookApproval?] = []

    func record(
        projectPath: String,
        worktree: Worktree,
        approval: WorktreeConfig.ProjectHookApproval?
    ) {
        projectPaths.append(projectPath)
        worktreeIDs.append(worktree.id)
        approvals.append(approval)
    }
}
