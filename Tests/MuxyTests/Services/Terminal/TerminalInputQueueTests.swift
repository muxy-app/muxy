import Foundation
import Testing

@testable import Muxy

@Suite("TerminalInputQueue")
@MainActor
struct TerminalInputQueueTests {
    @Test("preserves input order while an asynchronous operation is pending")
    func preservesInputOrder() async {
        let queue = TerminalInputQueue()
        var events: [String] = []

        queue.enqueue {
            events.append("upload")
            await Task.yield()
            events.append("path")
        }
        let deferred = queue.deferIfPending {
            events.append("return")
        }

        #expect(deferred)
        await queue.waitUntilIdle()
        #expect(events == ["upload", "path", "return"])
    }

    @Test("operations running inside the queue can emit input immediately")
    func activeOperationCanEmitInput() async {
        let queue = TerminalInputQueue()
        var events: [String] = []

        queue.enqueue {
            events.append("upload")
            let deferred = queue.deferIfPending {
                events.append("deferred-path")
            }
            #expect(!deferred)
            events.append("path")
        }
        _ = queue.deferIfPending {
            events.append("return")
        }

        await queue.waitUntilIdle()
        #expect(events == ["upload", "path", "return"])
    }

    @Test("perform keeps later input behind the complete transaction")
    func transactionOrdering() async {
        let queue = TerminalInputQueue()
        var events: [String] = []
        let (starts, startContinuation) = AsyncStream<Void>.makeStream()
        let (releases, releaseContinuation) = AsyncStream<Void>.makeStream()

        let transaction = Task { @MainActor in
            await queue.perform {
                events.append("text")
                startContinuation.yield()
                startContinuation.finish()
                for await _ in releases {
                    break
                }
                events.append("path")
                return true
            }
        }
        for await _ in starts {
            break
        }
        let deferred = queue.deferIfPending {
            events.append("return")
        }
        releaseContinuation.yield()
        releaseContinuation.finish()

        #expect(deferred)
        #expect(await transaction.value)
        await queue.waitUntilIdle()
        #expect(events == ["text", "path", "return"])
    }

    @Test("synchronous transaction handles register before an immediate follower")
    func synchronousTransactionRegistration() async {
        let queue = TerminalInputQueue()
        var events: [String] = []
        let (releases, releaseContinuation) = AsyncStream<Void>.makeStream()

        let handle = queue.enqueueTransaction {
            events.append("transaction")
            for await _ in releases {
                break
            }
            events.append("complete")
            return true
        }
        let deferred = queue.deferIfPending {
            events.append("follower")
        }
        releaseContinuation.yield()
        releaseContinuation.finish()

        #expect(deferred)
        #expect(await handle.value())
        await queue.waitUntilIdle()
        #expect(events == ["transaction", "complete", "follower"])
    }

    @Test("cancel all reaches the active operation when a follower is queued")
    func cancelsActiveOperation() async {
        let queue = TerminalInputQueue()
        var events: [String] = []
        let (starts, startContinuation) = AsyncStream<Void>.makeStream()

        queue.enqueue {
            events.append("upload")
            startContinuation.yield()
            startContinuation.finish()
            try? await Task.sleep(for: .seconds(30))
            events.append("cancelled")
        }
        for await _ in starts {
            break
        }
        _ = queue.deferIfPending {
            events.append("return")
        }

        let cancelledWorker = queue.cancelAll()
        await cancelledWorker?.value
        await queue.waitUntilIdle()

        #expect(events == ["upload", "cancelled"])
        #expect(!queue.hasPendingOperations)
    }
}
