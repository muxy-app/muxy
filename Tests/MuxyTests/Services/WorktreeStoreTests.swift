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

    @Test("refreshWorktrees imports missing external worktrees and preserves existing IDs by path")
    func refreshWorktreesImportsAndPreservesIDs() async throws {
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
        let listingStub = WorktreeListingStub(recordsByRepoPath: [
            project.path: [
                WorktreeRecord(
                    path: project.path,
                    branch: "main",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
                WorktreeRecord(
                    path: "/tmp/repo-feature-a",
                    branch: "feature-a",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
                WorktreeRecord(
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
            listWorktrees: listingStub.listWorktrees,
            projects: [project]
        )

        let worktrees = try await store.refreshWorktrees(project: project)

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

    @Test("refreshWorktrees keeps missing Muxy-managed worktrees")
    func refreshWorktreesKeepsMissingMuxyManagedEntries() async throws {
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
        let listingStub = WorktreeListingStub(recordsByRepoPath: [
            project.path: [
                WorktreeRecord(
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
            listWorktrees: listingStub.listWorktrees,
            projects: [project]
        )

        let worktrees = try await store.refreshWorktrees(project: project)

        #expect(worktrees.count == 2)
        #expect(worktrees.contains(where: { $0.path == "/tmp/repo-retained" }))
    }

    @Test("refreshWorktrees removes missing external worktrees")
    func refreshWorktreesRemovesMissingExternalEntries() async throws {
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
        let listingStub = WorktreeListingStub(recordsByRepoPath: [
            project.path: [
                WorktreeRecord(
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
            listWorktrees: listingStub.listWorktrees,
            projects: [project]
        )

        let worktrees = try await store.refreshWorktrees(project: project)

        #expect(worktrees.count == 1)
        #expect(worktrees.allSatisfy { !$0.isExternallyManaged })
        #expect(!worktrees.contains(where: { $0.path == "/tmp/repo-external" }))
    }

    @Test("refreshWorktrees ignores bare and prunable records")
    func refreshWorktreesIgnoresUnusableRecords() async throws {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let persistence = WorktreePersistenceStub(
            initial: [
                project.id: [
                    Worktree(name: project.name, path: project.path, isPrimary: true),
                ]
            ]
        )
        let listingStub = WorktreeListingStub(recordsByRepoPath: [
            project.path: [
                WorktreeRecord(
                    path: project.path,
                    branch: "main",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
                WorktreeRecord(
                    path: "/tmp/repo-bare",
                    branch: nil,
                    head: nil,
                    isBare: true,
                    isDetached: false
                ),
                WorktreeRecord(
                    path: "/tmp/repo-prunable",
                    branch: "feature-prunable",
                    head: nil,
                    isBare: false,
                    isDetached: false,
                    isPrunable: true
                ),
                WorktreeRecord(
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
            listWorktrees: listingStub.listWorktrees,
            projects: [project]
        )

        let worktrees = try await store.refreshWorktrees(project: project)

        #expect(worktrees.count == 2)
        #expect(worktrees.contains(where: { $0.path == "/tmp/repo-live" }))
        #expect(!worktrees.contains(where: { $0.path == "/tmp/repo-bare" }))
        #expect(!worktrees.contains(where: { $0.path == "/tmp/repo-prunable" }))
    }

    @Test("refreshWorktrees tolerates duplicate persisted paths without trapping")
    func refreshWorktreesToleratesDuplicatePaths() async throws {
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
        let listingStub = WorktreeListingStub(recordsByRepoPath: [
            project.path: [
                WorktreeRecord(
                    path: project.path,
                    branch: "main",
                    head: nil,
                    isBare: false,
                    isDetached: false
                ),
                WorktreeRecord(
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
            listWorktrees: listingStub.listWorktrees,
            projects: [project]
        )

        let worktrees = try await store.refreshWorktrees(project: project)

        let atDuplicatePath = worktrees.filter { $0.path == duplicatePath }
        #expect(atDuplicatePath.count == 2)
        #expect(atDuplicatePath.contains(where: { $0.branch == "updated" }))
    }

    @Test("refreshWorktrees treats symlinked primary paths as the primary worktree")
    func refreshWorktreesResolvesSymlinkedPrimaryPath() async throws {
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
        let listingStub = WorktreeListingStub(recordsByRepoPath: [
            project.path: [
                WorktreeRecord(
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
            listWorktrees: listingStub.listWorktrees,
            projects: [project]
        )

        let worktrees = try await store.refreshWorktrees(project: project)

        #expect(worktrees.count == 1)
        #expect(worktrees[0].isPrimary)
        #expect(worktrees[0].branch == "feat/worktree-refresh")
    }

    @Test("remove evicts cached VCS state for the removed worktree")
    func removeEvictsCachedVCSState() {
        let project = Project(name: "Repo", path: "/tmp/repo-\(UUID().uuidString)")
        let removable = Worktree(
            name: "feature-a",
            path: project.path + "-feature-a",
            branch: "feature-a",
            source: .muxy,
            isPrimary: false
        )
        let persistence = WorktreePersistenceStub(
            initial: [
                project.id: [
                    Worktree(name: project.name, path: project.path, isPrimary: true),
                    removable,
                ]
            ]
        )
        let store = WorktreeStore(
            persistence: persistence,
            listWorktrees: WorktreeListingStub(recordsByRepoPath: [:]).listWorktrees,
            projects: [project]
        )
        _ = VCSStateStore.shared.state(for: removable.path)
        #expect(VCSStateStore.shared.cachedState(for: removable.path) != nil)

        store.remove(worktreeID: removable.id, from: project.id)

        #expect(VCSStateStore.shared.cachedState(for: removable.path) == nil)
    }

    @Test("remove does not delete externally managed worktrees")
    func removeDoesNotDeleteExternalWorktree() {
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
            listWorktrees: WorktreeListingStub(recordsByRepoPath: [:]).listWorktrees,
            projects: [project]
        )

        store.remove(worktreeID: external.id, from: project.id)

        #expect(store.list(for: project.id).contains(external))
        #expect(external.canBeRemoved == false)
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
        #expect(external.toDTO().canBeRemoved == false)
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

private struct WorktreeListingStub {
    let recordsByRepoPath: [String: [WorktreeRecord]]

    func listWorktrees(repoPath: String) async throws -> [WorktreeRecord] {
        recordsByRepoPath[repoPath] ?? []
    }
}
