import Darwin
import Dispatch
import Foundation
import MuxyHookKit
import Testing

@Suite("Agent hook standard input")
struct AgentHookStandardInputTests {
    @Test("reads a payload smaller than the cap in full")
    func readsSmallPayload() throws {
        let payload = Data(#"{"message":"hello"}"#.utf8)
        let result = try readThroughPipe(payload: payload, limit: 1024)

        #expect(result == payload)
    }

    @Test("caps an oversized payload at the limit")
    func capsOversizedPayload() throws {
        let limit = 4096
        let payload = Data(repeating: UInt8(ascii: "a"), count: limit * 4)
        let result = try readThroughPipe(payload: payload, limit: limit)

        #expect(result.count == limit)
    }

    @Test("drains the remainder so the producer never blocks on a full pipe")
    func drainsRemainderWithoutBlockingProducer() throws {
        let limit = 1024
        let payload = Data(repeating: UInt8(ascii: "b"), count: 8 * 1024 * 1024)
        let writerFinished = DispatchSemaphore(value: 0)

        let result = try readThroughPipe(payload: payload, limit: limit) {
            writerFinished.signal()
        }

        #expect(result.count == limit)
        #expect(writerFinished.wait(timeout: .now() + 10) == .success)
    }

    private func readThroughPipe(
        payload: Data,
        limit: Int,
        onWriteCompleted: (@Sendable () -> Void)? = nil
    ) throws -> Data {
        var descriptors = [Int32](repeating: -1, count: 2)
        try #require(pipe(&descriptors) == 0)
        let readEnd = descriptors[0]
        let writeEnd = descriptors[1]

        DispatchQueue.global().async {
            payload.withUnsafeBytes { buffer in
                guard let baseAddress = buffer.baseAddress else { return }
                var offset = 0
                while offset < buffer.count {
                    let written = Darwin.write(writeEnd, baseAddress.advanced(by: offset), buffer.count - offset)
                    guard written > 0 else { break }
                    offset += written
                }
            }
            close(writeEnd)
            onWriteCompleted?()
        }

        let result = AgentHookStandardInput.read(descriptor: readEnd, limit: limit)
        close(readEnd)
        return result
    }
}
