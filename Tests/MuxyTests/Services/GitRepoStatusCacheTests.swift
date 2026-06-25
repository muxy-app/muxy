import Foundation
import Testing

@testable import Muxy

@Suite("GitRepoStatusCache")
@MainActor
struct GitRepoStatusCacheTests {
    private func makeCache() -> GitRepoStatusCache {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("git-repo-status-\(UUID().uuidString).json")
        return GitRepoStatusCache(store: CodableFileStore(fileURL: url))
    }

    private var remote: WorkspaceContext {
        .ssh(SSHDestination(host: "server"))
    }

    @Test("returns nil before any update")
    func missingReturnsNil() {
        let cache = makeCache()
        #expect(cache.cachedStatus(for: "/repo", context: .local) == nil)
    }

    @Test("stores and reads status per context")
    func storesPerContext() {
        let cache = makeCache()
        cache.update(path: "/repo", context: .local, isGitRepo: true)
        #expect(cache.cachedStatus(for: "/repo", context: .local) == true)
    }

    @Test("same path on different contexts does not collide")
    func contextsDoNotCollide() {
        let cache = makeCache()
        cache.update(path: "/repo", context: .local, isGitRepo: true)
        cache.update(path: "/repo", context: remote, isGitRepo: false)
        #expect(cache.cachedStatus(for: "/repo", context: .local) == true)
        #expect(cache.cachedStatus(for: "/repo", context: remote) == false)
    }

    @Test("remove clears only the matching entry")
    func removeClearsEntry() {
        let cache = makeCache()
        cache.update(path: "/repo", context: .local, isGitRepo: true)
        cache.update(path: "/repo", context: remote, isGitRepo: true)
        cache.remove(path: "/repo", context: .local)
        #expect(cache.cachedStatus(for: "/repo", context: .local) == nil)
        #expect(cache.cachedStatus(for: "/repo", context: remote) == true)
    }

    @Test("status persists across cache instances")
    func persistsAcrossInstances() {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("git-repo-status-\(UUID().uuidString).json")
        let first = GitRepoStatusCache(store: CodableFileStore(fileURL: url))
        first.update(path: "/repo", context: .local, isGitRepo: true)

        let second = GitRepoStatusCache(store: CodableFileStore(fileURL: url))
        #expect(second.cachedStatus(for: "/repo", context: .local) == true)
    }
}
