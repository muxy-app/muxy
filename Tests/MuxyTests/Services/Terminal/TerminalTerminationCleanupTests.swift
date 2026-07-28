import Foundation
import Testing

@testable import Muxy

@Suite("Terminal termination cleanup")
@MainActor
struct TerminalTerminationCleanupTests {
    @Test("completes after cleanup finishes")
    func completesAfterCleanup() async {
        let cleanup = TerminalTerminationCleanup()
        var events: [String] = []

        cleanup.start(
            timeout: .seconds(1),
            cleanup: {
                events.append("cleanup")
            },
            completion: {
                events.append("complete")
            }
        )
        while !cleanup.isComplete {
            await Task.yield()
        }

        #expect(events == ["cleanup", "complete"])
        #expect(!cleanup.isRunning)
    }

    @Test("cancels cleanup when the timeout expires")
    func cancelsAfterTimeout() async {
        let cleanup = TerminalTerminationCleanup()
        let (completions, completionContinuation) = AsyncStream<Void>.makeStream()

        cleanup.start(
            timeout: .milliseconds(10),
            cleanup: {
                try? await Task.sleep(for: .seconds(30))
            },
            completion: {
                completionContinuation.yield()
                completionContinuation.finish()
            }
        )
        for await _ in completions {
            break
        }

        #expect(cleanup.isComplete)
        #expect(!cleanup.isRunning)
    }
}
