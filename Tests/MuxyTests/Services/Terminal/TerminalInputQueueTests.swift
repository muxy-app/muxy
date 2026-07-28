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
}
