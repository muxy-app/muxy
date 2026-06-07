import Foundation
import Testing

@testable import Muxy

@MainActor
@Suite("ExtensionLogCoalescer")
struct ExtensionLogCoalescerTests {
    @Test("appends within one window are merged into a single update")
    func appendsAreMerged() async throws {
        var updates: [ExtensionLogUpdate] = []
        let coalescer = ExtensionLogCoalescer { updates.append($0) }

        coalescer.ingest(.append(["a"]))
        coalescer.ingest(.append(["b", "c"]))
        #expect(updates.isEmpty)

        try await settle()

        #expect(updates.count == 1)
        #expect(lines(of: updates[0]) == ["a", "b", "c"])
    }

    @Test("reset flushes immediately and drops pending appends")
    func resetDropsPendingAppends() async throws {
        var updates: [ExtensionLogUpdate] = []
        let coalescer = ExtensionLogCoalescer { updates.append($0) }

        coalescer.ingest(.append(["stale"]))
        coalescer.ingest(.reset(["fresh"]))

        #expect(updates.count == 1)
        #expect(isReset(updates[0]))
        #expect(lines(of: updates[0]) == ["fresh"])

        try await settle()

        #expect(updates.count == 1)
    }

    @Test("cancel discards pending appends")
    func cancelDiscardsPending() async throws {
        var updates: [ExtensionLogUpdate] = []
        let coalescer = ExtensionLogCoalescer { updates.append($0) }

        coalescer.ingest(.append(["dropped"]))
        coalescer.cancel()

        try await settle()

        #expect(updates.isEmpty)
    }

    private func settle() async throws {
        try await Task.sleep(for: .milliseconds(120))
    }

    private func lines(of update: ExtensionLogUpdate) -> [String] {
        switch update {
        case let .reset(lines), let .append(lines): lines
        }
    }

    private func isReset(_ update: ExtensionLogUpdate) -> Bool {
        if case .reset = update { return true }
        return false
    }
}
