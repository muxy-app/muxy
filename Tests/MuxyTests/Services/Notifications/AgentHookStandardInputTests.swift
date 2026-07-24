import Foundation
import MuxyHookKit
import Testing

@Suite("Agent hook standard input")
struct AgentHookStandardInputTests {
    @Test("reads a payload smaller than the cap in full")
    func readsSmallPayload() throws {
        let payload = Data(#"{"message":"hello"}"#.utf8)
        let result = try readFromFile(payload: payload, limit: 1024)

        #expect(result == payload)
    }

    @Test("caps an oversized payload at the limit")
    func capsOversizedPayload() throws {
        let limit = 4096
        let payload = Data(repeating: UInt8(ascii: "a"), count: limit * 4)
        let result = try readFromFile(payload: payload, limit: limit)

        #expect(result.count == limit)
    }

    private func readFromFile(payload: Data, limit: Int) throws -> Data {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("AgentHookStandardInputTests-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: url) }
        try payload.write(to: url)
        let handle = try FileHandle(forReadingFrom: url)
        defer { try? handle.close() }
        return AgentHookStandardInput.read(descriptor: handle.fileDescriptor, limit: limit)
    }
}
