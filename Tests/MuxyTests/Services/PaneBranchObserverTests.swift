import Foundation
import Testing

@testable import Muxy

private final class ResolverProbe: @unchecked Sendable {
    private let lock = NSLock()
    private var observed: [String] = []
    private var queue: [String?]?
    private var fixed: String?

    init(fixed: String? = nil) {
        self.fixed = fixed
    }

    init(queue: [String?]) {
        self.queue = queue
    }

    func record(_ path: String) -> String? {
        lock.lock()
        defer { lock.unlock() }
        observed.append(path)
        if queue != nil {
            guard let queue, !queue.isEmpty else { return nil }
            let next = queue.first!
            self.queue = Array(queue.dropFirst())
            return next
        }
        return fixed
    }

    var calls: Int {
        lock.lock()
        defer { lock.unlock() }
        return observed.count
    }

    var paths: [String] {
        lock.lock()
        defer { lock.unlock() }
        return observed
    }
}

@MainActor
@Suite("PaneBranchObserver")
struct PaneBranchObserverTests {
    private static let pollInterval: Duration = .milliseconds(20)
    private static let pollTimeout: Duration = .milliseconds(2000)

    private static func waitUntil(
        _ condition: @MainActor () -> Bool
    ) async {
        let deadline = ContinuousClock.now.advanced(by: pollTimeout)
        while !condition(), ContinuousClock.now < deadline {
            try? await Task.sleep(for: pollInterval)
        }
    }

    @Test("update with nil clears the branch and skips the resolver")
    func clearOnNilPath() async {
        let probe = ResolverProbe(fixed: "main")
        let observer = PaneBranchObserver { path in probe.record(path) }
        observer.update(repoPath: "/tmp/repo")
        await Self.waitUntil { observer.branch == "main" }
        #expect(observer.branch == "main")
        #expect(probe.calls == 1)

        observer.update(repoPath: nil)
        await Self.waitUntil { observer.branch == nil }
        #expect(observer.branch == nil)
        #expect(probe.calls == 1)
    }

    @Test("changing repoPath triggers a resolver call")
    func resolveOnRepoChange() async {
        let probe = ResolverProbe(queue: ["feature/a", "feature/b"])
        let observer = PaneBranchObserver { path in probe.record(path) }
        observer.update(repoPath: "/tmp/a")
        await Self.waitUntil { observer.branch == "feature/a" }
        #expect(observer.branch == "feature/a")

        observer.update(repoPath: "/tmp/b")
        await Self.waitUntil { observer.branch == "feature/b" }
        #expect(observer.branch == "feature/b")
        #expect(probe.paths == ["/tmp/a", "/tmp/b"])
    }

    @Test("repeating the same path is a no-op")
    func sameRepoPathNoOp() async {
        let probe = ResolverProbe(fixed: "main")
        let observer = PaneBranchObserver { path in probe.record(path) }
        observer.update(repoPath: "/tmp/repo")
        await Self.waitUntil { probe.calls == 1 }
        observer.update(repoPath: "/tmp/repo")
        try? await Task.sleep(for: .milliseconds(100))
        #expect(probe.calls == 1)
    }

    @Test("resolver returning nil yields a nil branch (e.g. detached HEAD)")
    func resolverReturnsNil() async {
        let probe = ResolverProbe(fixed: nil)
        let observer = PaneBranchObserver { path in probe.record(path) }
        observer.update(repoPath: "/tmp/detached")
        await Self.waitUntil { probe.calls == 1 }
        #expect(observer.branch == nil)
    }

    @Test("manual refresh re-queries the resolver")
    func manualRefresh() async {
        let probe = ResolverProbe(queue: ["one", "two"])
        let observer = PaneBranchObserver { path in probe.record(path) }
        observer.update(repoPath: "/tmp/repo")
        await Self.waitUntil { observer.branch == "one" }
        #expect(observer.branch == "one")

        observer.refresh()
        await Self.waitUntil { observer.branch == "two" }
        #expect(observer.branch == "two")
    }
}
