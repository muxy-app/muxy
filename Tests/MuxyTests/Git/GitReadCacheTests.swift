import Foundation
import Testing

@testable import Muxy

@Suite("GitMetadataCache read cache", .serialized)
struct GitReadCacheTests {
    @Test("serves a cached value while the signature matches")
    func cacheHit() {
        let cache = GitMetadataCache.shared
        let key = key("/repo/a")
        cache.invalidateReads(repoPath: "/repo/a")

        cache.storeRead(["main"], key: key, signature: "sig-1")
        let value: [String]? = cache.cachedRead(key, signature: "sig-1")

        #expect(value == ["main"])
    }

    @Test("misses when the signature changes (external commit)")
    func signatureInvalidation() {
        let cache = GitMetadataCache.shared
        let key = key("/repo/b")
        cache.invalidateReads(repoPath: "/repo/b")

        cache.storeRead(["main"], key: key, signature: "sig-1")
        let value: [String]? = cache.cachedRead(key, signature: "sig-2")

        #expect(value == nil)
    }

    @Test("explicit invalidation drops every entry for the repo")
    func explicitInvalidation() {
        let cache = GitMetadataCache.shared
        let status = GitMetadataCache.ReadKey(repoPath: "/repo/c", endpoint: "status", params: "")
        let branches = GitMetadataCache.ReadKey(repoPath: "/repo/c", endpoint: "branches", params: "")
        cache.storeRead(true, key: status, signature: "sig")
        cache.storeRead(["main"], key: branches, signature: "sig")

        cache.invalidateReads(repoPath: "/repo/c")

        let a: Bool? = cache.cachedRead(status, signature: "sig")
        let b: [String]? = cache.cachedRead(branches, signature: "sig")
        #expect(a == nil)
        #expect(b == nil)
    }

    @Test("evicts the oldest entries once the capacity is exceeded")
    func capacityEviction() {
        let cache = GitMetadataCache.shared
        cache.invalidateReads(repoPath: "/repo/cap")
        let keys = (0 ..< 200).map {
            GitMetadataCache.ReadKey(repoPath: "/repo/cap", endpoint: "diff", params: "file=\($0)")
        }
        for key in keys {
            cache.storeRead([key.params], key: key, signature: "sig")
        }

        let oldest: [String]? = cache.cachedRead(keys[0], signature: "sig")
        let newest: [String]? = cache.cachedRead(keys[199], signature: "sig")
        #expect(oldest == nil)
        #expect(newest == ["file=199"])
    }

    @Test("isolates pull request entries and invalidation by workspace context")
    func pullRequestContextIsolation() throws {
        let cache = GitMetadataCache.shared
        let repoPath = "/repo/pr-context-\(UUID().uuidString)"
        let branch = "feature"
        let headSha = "abc123"
        let localContext = WorkspaceContext.local
        let remoteContext = WorkspaceContext.ssh(SSHDestination(host: "example.test"))
        let localInfo = pullRequestInfo(number: 41)
        let remoteInfo = pullRequestInfo(number: 42)
        defer {
            cache.invalidatePRInfo(context: localContext, repoPath: repoPath)
            cache.invalidatePRInfo(context: remoteContext, repoPath: repoPath)
        }

        cache.storePRInfo(
            localInfo,
            context: localContext,
            repoPath: repoPath,
            branch: branch,
            headSha: headSha
        )
        cache.storePRInfo(
            remoteInfo,
            context: remoteContext,
            repoPath: repoPath,
            branch: branch,
            headSha: headSha
        )

        let localEntry = try #require(cache.cachedPRInfo(
            context: localContext,
            repoPath: repoPath,
            branch: branch,
            headSha: headSha
        ))
        let remoteEntry = try #require(cache.cachedPRInfo(
            context: remoteContext,
            repoPath: repoPath,
            branch: branch,
            headSha: headSha
        ))
        #expect(localEntry == localInfo)
        #expect(remoteEntry == remoteInfo)

        cache.invalidatePRInfo(context: localContext, repoPath: repoPath)

        #expect(cache.cachedPRInfo(
            context: localContext,
            repoPath: repoPath,
            branch: branch,
            headSha: headSha
        ) == nil)
        let retainedRemoteEntry = try #require(cache.cachedPRInfo(
            context: remoteContext,
            repoPath: repoPath,
            branch: branch,
            headSha: headSha
        ))
        #expect(retainedRemoteEntry == remoteInfo)
    }

    private func key(_ repoPath: String) -> GitMetadataCache.ReadKey {
        GitMetadataCache.ReadKey(repoPath: repoPath, endpoint: "branches", params: "")
    }

    private func pullRequestInfo(number: Int) -> GitRepositoryService.PRInfo {
        GitRepositoryService.PRInfo(
            url: "https://github.com/muxy-app/muxy/pull/\(number)",
            number: number,
            state: .open,
            isDraft: false,
            baseBranch: "main",
            mergeable: true,
            mergeStateStatus: .clean,
            checks: .init(status: .success, passing: 1, failing: 0, pending: 0, total: 1),
            isCrossRepository: false
        )
    }
}
