import Foundation
import Testing

@testable import Muxy

@Suite("Remote tmux session exit handling")
struct RemoteTmuxSessionExitHandlerTests {
    private let destination = SSHDestination(host: "example.com", remoteSessionMode: .tmux)

    @Test("closes only when the remote tmux session is absent")
    func closesAbsentSession() {
        #expect(RemoteTmuxSessionExitHandler.decision(lookup: .absent, attempt: 1, limit: 3) == .closePane)
        #expect(RemoteTmuxSessionExitHandler.decision(lookup: .absent, attempt: 4, limit: 3) == .closePane)
    }

    @Test("retries present unknown and unavailable sessions within the attempt budget")
    func retriesRecoverableLookupsWithinBudget() {
        for lookup: RemoteTmuxLookup in [.present, .unknown, .unavailable] {
            #expect(RemoteTmuxSessionExitHandler.decision(lookup: lookup, attempt: 3, limit: 3) == .reattach)
        }
    }

    @Test("reports unknown and unavailable lookups after the attempt budget")
    func reportsUnresolvedLookupsAfterBudget() {
        for lookup: RemoteTmuxLookup in [.unknown, .unavailable] {
            #expect(RemoteTmuxSessionExitHandler.decision(lookup: lookup, attempt: 4, limit: 3) == .reportFailure)
        }
    }

    @Test("increments attempts inside the reset interval and resets at its boundary")
    func resetsAttemptsAfterInterval() {
        #expect(RemoteTmuxSessionExitHandler.attempt(previous: 2, elapsed: 59, resetAfter: 60) == 3)
        #expect(RemoteTmuxSessionExitHandler.attempt(previous: 3, elapsed: 60, resetAfter: 60) == 1)
        #expect(RemoteTmuxSessionExitHandler.attempt(previous: 3, elapsed: nil, resetAfter: 60) == 1)
    }

    @Test("uses increasing bounded recovery backoff")
    func calculatesBackoff() {
        #expect(RemoteTmuxSessionExitHandler.backoff(attempt: 0) == .milliseconds(300))
        #expect(RemoteTmuxSessionExitHandler.backoff(attempt: 3) == .milliseconds(900))
    }

    @MainActor
    @Test("reset recovery cannot clear a replacement task")
    func resetPreservesReplacementTask() async {
        let paneID = UUID()
        let session = RemoteTmuxSession(destination: destination)
        var lookups: [CheckedContinuation<RemoteTmuxLookup, Never>] = []
        let handler = RemoteTmuxSessionExitHandler(
            lookup: { _ in
                await withCheckedContinuation { lookups.append($0) }
            },
            hasSessionBacking: { _, _ in true },
            recoverySurface: { _ in nil }
        )

        let firstTask = handler.handleExit(paneID: paneID, session: session) {}
        await waitForLookupCount(1) { lookups.count }
        handler.resetPane(paneID)
        let replacementTask = handler.handleExit(paneID: paneID, session: session) {}
        await waitForLookupCount(2) { lookups.count }

        lookups[0].resume(returning: .unknown)
        await firstTask?.value
        #expect(handler.hasRecoveryTask(for: paneID))

        lookups[1].resume(returning: .absent)
        await replacementTask?.value
        #expect(!handler.hasRecoveryTask(for: paneID))
    }

    @MainActor
    private func waitForLookupCount(_ count: Int, lookupCount: () -> Int) async {
        while lookupCount() < count {
            await Task.yield()
        }
    }
}
