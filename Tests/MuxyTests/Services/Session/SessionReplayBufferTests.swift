import MuxySessionProtocol
import Testing

@Suite("SessionReplayBuffer")
struct SessionReplayBufferTests {
    @Test("starts empty")
    func startsEmpty() {
        let buffer = SessionReplayBuffer(capacity: 8)
        #expect(buffer.isEmpty)
        #expect(buffer.bytes.isEmpty)
        #expect(buffer.byteCount == 0)
    }

    @Test("keeps everything while below capacity")
    func keepsEverythingBelowCapacity() {
        var buffer = SessionReplayBuffer(capacity: 8)
        buffer.append([1, 2, 3])
        #expect(buffer.bytes == [1, 2, 3])
        buffer.append([4, 5])
        #expect(buffer.bytes == [1, 2, 3, 4, 5])
        #expect(buffer.byteCount == 5)
    }

    @Test("fills exactly to capacity without dropping")
    func fillsExactly() {
        var buffer = SessionReplayBuffer(capacity: 4)
        buffer.append([1, 2, 3, 4])
        #expect(buffer.bytes == [1, 2, 3, 4])
    }

    @Test("drops the oldest bytes once full")
    func dropsOldest() {
        var buffer = SessionReplayBuffer(capacity: 4)
        buffer.append([1, 2, 3, 4])
        buffer.append([5])
        #expect(buffer.bytes == [2, 3, 4, 5])
        buffer.append([6, 7])
        #expect(buffer.bytes == [4, 5, 6, 7])
    }

    @Test("keeps only the tail of a write larger than capacity")
    func keepsTailOfOversizedWrite() {
        var buffer = SessionReplayBuffer(capacity: 3)
        buffer.append([1, 2, 3, 4, 5, 6, 7])
        #expect(buffer.bytes == [5, 6, 7])
    }

    @Test("keeps only the tail of a write exactly one over capacity")
    func keepsTailOfBoundaryWrite() {
        var buffer = SessionReplayBuffer(capacity: 3)
        buffer.append([1, 2, 3, 4])
        #expect(buffer.bytes == [2, 3, 4])
    }

    @Test("stays correct across many wraparounds")
    func staysCorrectAcrossWraparounds() {
        var buffer = SessionReplayBuffer(capacity: 5)
        for value in UInt8(1) ... UInt8(50) {
            buffer.append([value])
        }
        #expect(buffer.bytes == [46, 47, 48, 49, 50])
    }

    @Test("wraps correctly when an oversized write follows a partial fill")
    func wrapsAfterPartialFill() {
        var buffer = SessionReplayBuffer(capacity: 4)
        buffer.append([1, 2, 3])
        buffer.append([4, 5, 6, 7, 8])
        #expect(buffer.bytes == [5, 6, 7, 8])
    }

    @Test("ignores empty writes and tolerates zero capacity")
    func toleratesDegenerateInput() {
        var buffer = SessionReplayBuffer(capacity: 4)
        buffer.append([])
        #expect(buffer.isEmpty)

        var zero = SessionReplayBuffer(capacity: 0)
        zero.append([1, 2, 3])
        #expect(zero.bytes.isEmpty)
    }

    @Test("clears back to empty")
    func clears() {
        var buffer = SessionReplayBuffer(capacity: 4)
        buffer.append([1, 2, 3, 4, 5])
        buffer.removeAll()
        #expect(buffer.isEmpty)
        buffer.append([9])
        #expect(buffer.bytes == [9])
    }
}
