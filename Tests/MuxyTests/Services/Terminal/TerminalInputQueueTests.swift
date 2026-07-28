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

        let transaction = Task { @MainActor in
            await queue.perform {
                events.append("text")
                await Task.yield()
                events.append("path")
                return true
            }
        }
        while events.isEmpty {
            await Task.yield()
        }
        let deferred = queue.deferIfPending {
            events.append("return")
        }

        #expect(deferred)
        #expect(await transaction.value)
        await queue.waitUntilIdle()
        #expect(events == ["text", "path", "return"])
    }

    @Test("cancel all reaches the active operation when a follower is queued")
    func cancelsActiveOperation() async {
        let queue = TerminalInputQueue()
        var events: [String] = []

        queue.enqueue {
            events.append("upload")
            while !Task.isCancelled {
                await Task.yield()
            }
            events.append("cancelled")
        }
        while events.isEmpty {
            await Task.yield()
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
