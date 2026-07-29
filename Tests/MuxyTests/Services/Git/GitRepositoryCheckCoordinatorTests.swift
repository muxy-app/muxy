import Foundation
import Testing

@testable import Muxy

@Suite("GitRepositoryCheckCoordinator")
struct GitRepositoryCheckCoordinatorTests {
    @Test("deduplicates concurrent checks for the same repository")
    func deduplicatesConcurrentChecks() async {
        let probe = GitRepositoryCheckProbe()
        let coordinator = makeCoordinator(maxConcurrentChecksPerContext: 4, probe: probe)

        async let checkResults = results(
            for: Array(repeating: ("/repo", WorkspaceContext.local), count: 8),
            coordinator: coordinator
        )
        #expect(await probe.waitForActiveChecks(1))
        try? await Task.sleep(for: .milliseconds(50))
        await probe.releaseAll()

        #expect(await checkResults.allSatisfy { $0 })
        #expect(await probe.checkCount == 1)
    }

    @Test("treats workspace contexts as distinct repositories")
    func separatesWorkspaceContexts() async {
        let probe = GitRepositoryCheckProbe()
        let coordinator = makeCoordinator(maxConcurrentChecksPerContext: 4, probe: probe)
        let remote = WorkspaceContext.ssh(SSHDestination(host: "server"))

        async let checkResults = results(
            for: [("/repo", WorkspaceContext.local), ("/repo", remote)],
            coordinator: coordinator
        )
        #expect(await probe.waitForActiveChecks(2))
        await probe.releaseAll()

        #expect(await checkResults.allSatisfy { $0 })
        #expect(await probe.checkCount == 2)
    }

    @Test("limits concurrent checks per workspace context")
    func limitsConcurrentChecks() async {
        let probe = GitRepositoryCheckProbe()
        let coordinator = makeCoordinator(maxConcurrentChecksPerContext: 2, probe: probe)
        let repositories = (0 ..< 8).map { ("/repo-\($0)", WorkspaceContext.local) }

        async let checkResults = results(for: repositories, coordinator: coordinator)
        #expect(await probe.waitForActiveChecks(2))
        await probe.releaseAll()

        #expect(await checkResults.allSatisfy { $0 })
        #expect(await probe.maximumActiveCheckCount == 2)
        #expect(await probe.checkCount == 8)
    }

    @Test("keeps local checks running while remote checks saturate their pool")
    func remoteChecksDoNotStarveLocalChecks() async {
        let probe = GitRepositoryCheckProbe()
        let coordinator = makeCoordinator(maxConcurrentChecksPerContext: 2, probe: probe)
        let remote = WorkspaceContext.ssh(SSHDestination(host: "server"))
        let repositories = [
            ("/remote-0", remote),
            ("/remote-1", remote),
            ("/remote-2", remote),
            ("/local", WorkspaceContext.local),
        ]

        async let checkResults = results(for: repositories, coordinator: coordinator)
        #expect(await probe.waitForActiveChecks(3))
        #expect(await probe.activePaths.contains("/local"))
        await probe.releaseAll()

        #expect(await checkResults.allSatisfy { $0 })
    }

    @Test("cancels the underlying check once every caller is cancelled")
    func cancelsCheckWhenAllCallersCancel() async {
        let probe = GitRepositoryCheckProbe()
        let coordinator = makeCoordinator(maxConcurrentChecksPerContext: 2, probe: probe)

        let caller = Task { await coordinator.isGitRepository("/repo", context: .local) }
        #expect(await probe.waitForActiveChecks(1))
        caller.cancel()

        #expect(await caller.value == false)
        #expect(await probe.cancelledCheckCount == 1)
    }

    @Test("keeps the check running while another caller is still waiting")
    func keepsCheckRunningForRemainingCallers() async {
        let probe = GitRepositoryCheckProbe()
        let coordinator = makeCoordinator(maxConcurrentChecksPerContext: 2, probe: probe)

        let cancelledCaller = Task { await coordinator.isGitRepository("/repo", context: .local) }
        #expect(await probe.waitForActiveChecks(1))
        let remainingCaller = Task { await coordinator.isGitRepository("/repo", context: .local) }
        try? await Task.sleep(for: .milliseconds(50))
        cancelledCaller.cancel()
        await probe.releaseAll()

        #expect(await remainingCaller.value)
        #expect(await probe.cancelledCheckCount == 0)
        #expect(await probe.checkCount == 1)
    }

    @Test("re-runs the check once the previous one finished")
    func doesNotCacheCompletedChecks() async {
        let probe = GitRepositoryCheckProbe()
        await probe.releaseAll()
        let coordinator = makeCoordinator(maxConcurrentChecksPerContext: 2, probe: probe)

        _ = await coordinator.isGitRepository("/repo", context: .local)
        _ = await coordinator.isGitRepository("/repo", context: .local)

        #expect(await probe.checkCount == 2)
    }

    private func makeCoordinator(
        maxConcurrentChecksPerContext: Int,
        probe: GitRepositoryCheckProbe
    ) -> GitRepositoryCheckCoordinator {
        GitRepositoryCheckCoordinator(
            maxConcurrentChecksPerContext: maxConcurrentChecksPerContext
        ) { path, context in
            await probe.check(path: path, context: context)
        }
    }

    private func results(
        for repositories: [(String, WorkspaceContext)],
        coordinator: GitRepositoryCheckCoordinator
    ) async -> [Bool] {
        await withTaskGroup(of: Bool.self, returning: [Bool].self) { group in
            for (path, context) in repositories {
                group.addTask {
                    await coordinator.isGitRepository(path, context: context)
                }
            }

            var results: [Bool] = []
            for await result in group {
                results.append(result)
            }
            return results
        }
    }
}

private actor GitRepositoryCheckProbe {
    private(set) var checkCount = 0
    private(set) var maximumActiveCheckCount = 0
    private(set) var cancelledCheckCount = 0
    private(set) var activePaths: [String] = []
    private var gatedChecks: [UUID: CheckedContinuation<Void, Never>] = [:]
    private var isReleased = false

    func check(path: String, context _: WorkspaceContext) async -> Bool {
        checkCount += 1
        activePaths.append(path)
        maximumActiveCheckCount = max(maximumActiveCheckCount, activePaths.count)
        await waitForRelease()
        activePaths.removeAll { $0 == path }
        return !Task.isCancelled
    }

    func waitForActiveChecks(_ target: Int, timeout: Duration = .seconds(5)) async -> Bool {
        let deadline = ContinuousClock.now + timeout
        while activePaths.count < target {
            guard ContinuousClock.now < deadline else { return false }
            try? await Task.sleep(for: .milliseconds(1))
        }
        return true
    }

    func releaseAll() {
        isReleased = true
        let continuations = Array(gatedChecks.values)
        gatedChecks.removeAll()
        for continuation in continuations {
            continuation.resume()
        }
    }

    private func waitForRelease() async {
        guard !isReleased else { return }
        let id = UUID()
        await withTaskCancellationHandler {
            await withCheckedContinuation { continuation in
                gatedChecks[id] = continuation
            }
        } onCancel: {
            Task { await self.releaseCancelledCheck(id: id) }
        }
    }

    private func releaseCancelledCheck(id: UUID) {
        guard let continuation = gatedChecks.removeValue(forKey: id) else { return }
        cancelledCheckCount += 1
        continuation.resume()
    }
}
