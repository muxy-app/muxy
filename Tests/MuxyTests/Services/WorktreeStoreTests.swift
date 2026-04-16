import Foundation
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

    @Test("refreshFromGit keeps stored worktrees that are absent from the latest Git listing")
    func refreshFromGitKeepsMissingStoredEntries() async throws {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let persistence = WorktreePersistenceStub(
            initial: [
                project.id: [
                    Worktree(name: project.name, path: project.path, isPrimary: true),
                    Worktree(
                        name: "Retained",
                        path: "/tmp/repo-retained",
                        branch: "retained",
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

        #expect(worktrees.count == 2)
        #expect(worktrees.contains(where: { $0.path == "/tmp/repo-retained" }))
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
            listGitWorktrees: GitWorktreeListingStub(recordsByRepoPath: [:]).listWorktrees,
            projects: [project]
        )

        store.remove(worktreeID: external.id, from: project.id)

        #expect(store.list(for: project.id).contains(external))
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
