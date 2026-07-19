import Foundation
import MuxyShared
import Testing

@Suite("Agent hook protocol v3")
struct AgentHookProtocolTests {
    @Test("event wire encoding is newline delimited and round trips every field")
    func eventRoundTrip() throws {
        let message = AgentHookEventMessage(
            provider: "claude_hook",
            paneID: UUID().uuidString,
            phase: .waiting,
            title: "Claude Code",
            body: "Allow command?",
            pids: [91, 42],
            ts: 1_721_234_567
        )

        let line = try AgentHookWireCodec.encodeEventLine(message)

        #expect(line.last == UInt8(ascii: "\n"))
        #expect(try AgentHookWireCodec.decodeEventLine(line) == message)
        let object = try #require(JSONSerialization.jsonObject(with: line.dropLast()) as? [String: Any])
        #expect(object["v"] as? Int == 3)
        #expect(object["kind"] as? String == "agent_event")
        #expect(object["provider"] as? String == "claude_hook")
        #expect(object["paneID"] as? String == message.paneID)
        #expect(object["pids"] as? [Int] == [91, 42])
        #expect(object["ts"] as? Int == 1_721_234_567)
    }

    @Test("missing pane identity is represented by an omitted paneID and an ancestor chain")
    func eventWithoutPaneIdentity() throws {
        let message = AgentHookEventMessage(
            provider: "pi",
            paneID: nil,
            phase: .finished,
            title: "",
            body: "",
            pids: [123, 45, 1],
            ts: 5
        )
        let line = try AgentHookWireCodec.encodeEventLine(message)
        let object = try #require(JSONSerialization.jsonObject(with: line.dropLast()) as? [String: Any])

        #expect(object["paneID"] == nil)
        #expect(try AgentHookWireCodec.decodeEventLine(line).pids == [123, 45, 1])
    }

    @Test("test flag is omitted when false and preserved when true")
    func testFlagEncoding() throws {
        let normal = AgentHookEventMessage(
            provider: "claude_hook",
            paneID: nil,
            phase: .finished,
            title: "t",
            body: "b",
            pids: [],
            ts: 1
        )
        let normalLine = try AgentHookWireCodec.encodeEventLine(normal)
        let normalObject = try #require(JSONSerialization.jsonObject(with: normalLine.dropLast()) as? [String: Any])
        #expect(normalObject["test"] == nil)
        #expect(try AgentHookWireCodec.decodeEventLine(normalLine).test == false)

        let test = AgentHookEventMessage(
            provider: "claude_hook",
            paneID: nil,
            phase: .finished,
            title: "t",
            body: "b",
            pids: [],
            ts: 1,
            test: true
        )
        let testLine = try AgentHookWireCodec.encodeEventLine(test)
        let testObject = try #require(JSONSerialization.jsonObject(with: testLine.dropLast()) as? [String: Any])
        #expect(testObject["test"] as? Bool == true)
        #expect(try AgentHookWireCodec.decodeEventLine(testLine).test == true)
    }

    @Test("legacy events without a test field decode as non-test")
    func legacyEventDecodesAsNonTest() throws {
        let legacy = #"{"body":"b","kind":"agent_event","phase":"finished","pids":[],"provider":"pi","title":"t","ts":1,"v":3}"#
        let message = try AgentHookWireCodec.decodeEventLine(Data(legacy.utf8))
        #expect(message.test == false)
    }

    @Test("acknowledgement encoding is newline delimited and round trips")
    func acknowledgementRoundTrip() throws {
        let acknowledgement = AgentHookAcknowledgement(ok: true)
        let line = try AgentHookWireCodec.encodeAcknowledgementLine(acknowledgement)

        #expect(line.last == UInt8(ascii: "\n"))
        #expect(try AgentHookWireCodec.decodeAcknowledgementLine(line) == acknowledgement)
        #expect(try AgentHookWireCodec.decodeAcknowledgementLine(line.dropLast()) == acknowledgement)
    }
}
