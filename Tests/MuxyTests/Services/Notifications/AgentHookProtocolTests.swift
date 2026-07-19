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

    @Test("acknowledgement encoding is newline delimited and round trips")
    func acknowledgementRoundTrip() throws {
        let acknowledgement = AgentHookAcknowledgement(ok: true)
        let line = try AgentHookWireCodec.encodeAcknowledgementLine(acknowledgement)

        #expect(line.last == UInt8(ascii: "\n"))
        #expect(try AgentHookWireCodec.decodeAcknowledgementLine(line) == acknowledgement)
        #expect(try AgentHookWireCodec.decodeAcknowledgementLine(line.dropLast()) == acknowledgement)
    }
}
