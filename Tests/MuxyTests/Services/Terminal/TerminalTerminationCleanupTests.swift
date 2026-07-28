import Foundation
import Testing

@testable import Muxy

@Suite("Terminal termination cleanup")
@MainActor
struct TerminalTerminationCleanupTests {
    @Test("completes after cleanup finishes")
    func completesAfterCleanup() async {
        let cleanup = TerminalTerminationCleanup()
        let (completions, completionContinuation) = AsyncStream<Void>.makeStream()
        var events: [String] = []

        cleanup.start(
            timeout: .seconds(1),
            cleanup: {
                events.append("cleanup")
            },
            completion: {
                events.append("complete")
                completionContinuation.yield()
                completionContinuation.finish()
            }
        )
        for await _ in completions {
            break
        }

        #expect(events == ["cleanup", "complete"])
        #expect(cleanup.isComplete)
        #expect(!cleanup.isRunning)
    }

    @Test("replies to every termination request made while cleanup runs")
    func repliesToConcurrentTerminationRequests() async {
        let cleanup = TerminalTerminationCleanup()
        let (releases, releaseContinuation) = AsyncStream<Void>.makeStream()
        var replies = 0

        for _ in 0 ..< 3 {
            cleanup.start(
                timeout: .seconds(5),
                cleanup: {
                    for await _ in releases {
                        break
                    }
                },
                completion: {
                    replies += 1
                }
            )
        }

        #expect(replies == 0)
        releaseContinuation.yield()
        releaseContinuation.finish()
        while !cleanup.isComplete {
            await Task.yield()
        }

        #expect(replies == 3)
    }

    @Test("replies immediately once cleanup has already completed")
    func repliesAfterCompletion() async {
        let cleanup = TerminalTerminationCleanup()
        let (completions, completionContinuation) = AsyncStream<Void>.makeStream()
        var replies = 0

        cleanup.start(
            timeout: .seconds(1),
            cleanup: {},
            completion: {
                replies += 1
                completionContinuation.yield()
                completionContinuation.finish()
            }
        )
        for await _ in completions {
            break
        }

        cleanup.start(timeout: .seconds(1), cleanup: {}, completion: { replies += 1 })

        #expect(replies == 2)
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

    @Test("tracks cleanup after its pane has been removed")
    func tracksRemovedPaneCleanup() async {
        let coordinator = TerminalCleanupCoordinator()
        let (starts, startContinuation) = AsyncStream<Void>.makeStream()
        let (releases, releaseContinuation) = AsyncStream<Void>.makeStream()
        var didFinish = false

        coordinator.schedule {
            startContinuation.yield()
            startContinuation.finish()
            for await _ in releases {
                break
            }
            didFinish = true
        }
        for await _ in starts {
            break
        }

        #expect(coordinator.outstandingCount == 1)
        releaseContinuation.yield()
        releaseContinuation.finish()
        await coordinator.waitUntilIdle()

        #expect(didFinish)
        #expect(coordinator.outstandingCount == 0)
    }

    @Test("termination timeout cancels centrally tracked cleanup")
    func timeoutCancelsTrackedCleanup() async {
        let coordinator = TerminalCleanupCoordinator()
        let terminationCleanup = TerminalTerminationCleanup()
        let (starts, startContinuation) = AsyncStream<Void>.makeStream()
        let (cancellations, cancellationContinuation) = AsyncStream<Void>.makeStream()
        let (completions, completionContinuation) = AsyncStream<Void>.makeStream()

        coordinator.schedule {
            startContinuation.yield()
            startContinuation.finish()
            do {
                try await Task.sleep(for: .seconds(30))
            } catch {
                cancellationContinuation.yield()
                cancellationContinuation.finish()
            }
        }
        for await _ in starts {
            break
        }

        terminationCleanup.start(
            timeout: .milliseconds(10),
            cleanup: {
                await coordinator.waitUntilIdle()
            },
            completion: {
                completionContinuation.yield()
                completionContinuation.finish()
            }
        )
        for await _ in completions {
            break
        }
        for await _ in cancellations {
            break
        }

        #expect(terminationCleanup.isComplete)
        #expect(coordinator.outstandingCount == 0)
    }
}
