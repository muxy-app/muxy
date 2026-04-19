import Foundation
import Testing

@testable import Muxy

@Suite("DiffCache")
@MainActor
struct DiffCacheTests {
    private func makeDiff(additions: Int = 1, deletions: Int = 0, truncated: Bool = false) -> DiffCache.LoadedDiff {
        DiffCache.LoadedDiff(rows: [], additions: additions, deletions: deletions, truncated: truncated)
    }

    @Test("store makes diff retrievable")
    func storeMakesDiffRetrievable() {
        let cache = DiffCache()
        let diff = makeDiff()
        cache.store(diff, for: "a.swift", pinnedPaths: [])
        #expect(cache.hasDiff(for: "a.swift"))
        #expect(cache.diff(for: "a.swift")?.additions == 1)
    }

    @Test("markLoading reports loading and clears prior error")
    func markLoadingClearsError() {
        let cache = DiffCache()
        cache.storeError("boom", for: "a.swift")
        #expect(cache.error(for: "a.swift") == "boom")
        cache.markLoading("a.swift")
        #expect(cache.isLoading("a.swift"))
        #expect(cache.error(for: "a.swift") == nil)
    }

    @Test("store clears loading state for that path")
    func storeClearsLoading() {
        let cache = DiffCache()
        cache.markLoading("a.swift")
        #expect(cache.isLoading("a.swift"))
        cache.store(makeDiff(), for: "a.swift", pinnedPaths: [])
        #expect(!cache.isLoading("a.swift"))
    }

    @Test("storeError clears loading state")
    func storeErrorClearsLoading() {
        let cache = DiffCache()
        cache.markLoading("a.swift")
        cache.storeError("oops", for: "a.swift")
        #expect(!cache.isLoading("a.swift"))
        #expect(cache.error(for: "a.swift") == "oops")
    }

    @Test("evict removes diff, error, loading, and access order entry")
    func evictRemovesAll() {
        let cache = DiffCache()
        cache.store(makeDiff(), for: "a.swift", pinnedPaths: [])
        cache.storeError("e", for: "a.swift")
        cache.markLoading("a.swift")
        cache.evict("a.swift")
        #expect(!cache.hasDiff(for: "a.swift"))
        #expect(cache.error(for: "a.swift") == nil)
        #expect(!cache.isLoading("a.swift"))
    }

    @Test("LRU evicts oldest unpinned entry when cap exceeded")
    func lruEvictsOldestUnpinned() {
        let cache = DiffCache(cap: 2)
        cache.store(makeDiff(), for: "a.swift", pinnedPaths: [])
        cache.store(makeDiff(), for: "b.swift", pinnedPaths: [])
        cache.store(makeDiff(), for: "c.swift", pinnedPaths: [])
        #expect(!cache.hasDiff(for: "a.swift"))
        #expect(cache.hasDiff(for: "b.swift"))
        #expect(cache.hasDiff(for: "c.swift"))
    }

    @Test("LRU keeps pinned entries even when they are oldest")
    func lruRespectsPinned() {
        let cache = DiffCache(cap: 2)
        cache.store(makeDiff(), for: "a.swift", pinnedPaths: ["a.swift"])
        cache.store(makeDiff(), for: "b.swift", pinnedPaths: ["a.swift"])
        cache.store(makeDiff(), for: "c.swift", pinnedPaths: ["a.swift"])
        #expect(cache.hasDiff(for: "a.swift"))
        #expect(cache.hasDiff(for: "b.swift"))
        #expect(cache.hasDiff(for: "c.swift"))
    }

    @Test("touch moves entry to newest in LRU")
    func touchMovesToNewest() {
        let cache = DiffCache(cap: 2)
        cache.store(makeDiff(), for: "a.swift", pinnedPaths: [])
        cache.store(makeDiff(), for: "b.swift", pinnedPaths: [])
        cache.touch("a.swift")
        cache.store(makeDiff(), for: "c.swift", pinnedPaths: [])
        #expect(cache.hasDiff(for: "a.swift"))
        #expect(!cache.hasDiff(for: "b.swift"))
        #expect(cache.hasDiff(for: "c.swift"))
    }

    @Test("collapseAll clears loading and errors but keeps diffs")
    func collapseAllKeepsDiffs() {
        let cache = DiffCache()
        cache.store(makeDiff(), for: "a.swift", pinnedPaths: [])
        cache.markLoading("b.swift")
        cache.storeError("e", for: "c.swift")
        cache.collapseAll()
        #expect(cache.hasDiff(for: "a.swift"))
        #expect(!cache.isLoading("b.swift"))
        #expect(cache.error(for: "c.swift") == nil)
    }

    @Test("clearAll removes everything")
    func clearAllRemovesEverything() {
        let cache = DiffCache()
        cache.store(makeDiff(), for: "a.swift", pinnedPaths: [])
        cache.markLoading("b.swift")
        cache.storeError("e", for: "c.swift")
        cache.clearAll()
        #expect(!cache.hasDiff(for: "a.swift"))
        #expect(!cache.isLoading("b.swift"))
        #expect(cache.error(for: "c.swift") == nil)
    }

    @Test("cancelLoad clears loading flag for path")
    func cancelLoadClearsLoading() {
        let cache = DiffCache()
        cache.markLoading("a.swift")
        cache.cancelLoad(for: "a.swift")
        #expect(!cache.isLoading("a.swift"))
    }
}
