import Foundation
import Testing

@testable import Muxy

@Suite("DiffLoadGate")
struct DiffLoadGateTests {
    @Test("acquire grants slots up to the limit immediately")
    func acquireGrantsUntilLimit() async throws {
        let gate = DiffLoadGate(limit: 2)
        try await gate.acquire()
        try await gate.acquire()
        await gate.release()
        await gate.release()
    }

    @Test("release hands slot to next waiter")
    func releaseHandsToWaiter() async throws {
        let gate = DiffLoadGate(limit: 1)
        try await gate.acquire()

        let waiter = Task {
            try await gate.acquire()
            await gate.release()
        }

        try await Task.sleep(nanoseconds: 50_000_000)
        await gate.release()
        try await waiter.value
    }

    @Test("cancelled waiter does not leak a slot")
    func cancelledWaiterDoesNotLeakSlot() async throws {
        let gate = DiffLoadGate(limit: 1)
        try await gate.acquire()

        let cancelled = Task {
            do {
                try await gate.acquire()
                Issue.record("expected cancellation")
            } catch {}
        }

        try await Task.sleep(nanoseconds: 50_000_000)
        cancelled.cancel()
        await cancelled.value

        await gate.release()

        try await withThrowingTaskGroup(of: Void.self) { group in
            group.addTask {
                try await gate.acquire()
                await gate.release()
            }
            try await group.waitForAll()
        }
    }

    @Test("many cancelled waiters do not exhaust the gate")
    func manyCancelledWaitersDoNotExhaustGate() async throws {
        let gate = DiffLoadGate(limit: 1)
        try await gate.acquire()

        for _ in 0 ..< 16 {
            let waiter = Task {
                do {
                    try await gate.acquire()
                    await gate.release()
                } catch {}
            }
            try await Task.sleep(nanoseconds: 5_000_000)
            waiter.cancel()
            await waiter.value
        }

        await gate.release()

        try await withThrowingTaskGroup(of: Void.self) { group in
            for _ in 0 ..< 4 {
                group.addTask {
                    try await gate.acquire()
                    await gate.release()
                }
            }
            try await group.waitForAll()
        }
    }
}
