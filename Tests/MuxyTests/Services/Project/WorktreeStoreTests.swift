import Foundation
import MuxyShared
import Testing

@testable import Muxy

@Suite("WorktreeStore")
@MainActor
struct WorktreeStoreTests {
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
        let store = WorktreeStore(persistence: WorktreePersistenceStub(initial: [:]))
        let worktree = Worktree(
            name: "feature",
            path: "/tmp/repo-feature",
            isPrimary: false
        )

        #expect(store.beginRemovalPreparation(worktree: worktree))
        #expect(!store.beginRemovalPreparation(worktree: worktree))
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

        #expect(store.beginRemovalPreparation(worktree: worktree))
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
        _ = store.beginRemovalPreparation(worktree: worktree)

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
            baseBranch: nil
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
