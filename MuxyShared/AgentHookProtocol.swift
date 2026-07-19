import Foundation

public enum AgentHookProtocol {
    public static let version = 3
    public static let eventKind = "agent_event"
    public static let acknowledgementKind = "ack"
}

public enum AgentHookPhase: String, Codable, Equatable, Sendable {
    case working
    case waiting
    case finished
}

public struct AgentHookEventMessage: Codable, Equatable, Sendable {
    public let v: Int
    public let kind: String
    public let provider: String
    public let paneID: String?
    public let phase: AgentHookPhase
    public let title: String
    public let body: String
    public let pids: [Int32]
    public let ts: Int64

    public init(
        v: Int = AgentHookProtocol.version,
        kind: String = AgentHookProtocol.eventKind,
        provider: String,
        paneID: String?,
        phase: AgentHookPhase,
        title: String,
        body: String,
        pids: [Int32],
        ts: Int64
    ) {
        self.v = v
        self.kind = kind
        self.provider = provider
        self.paneID = paneID
        self.phase = phase
        self.title = title
        self.body = body
        self.pids = pids
        self.ts = ts
    }
}

public struct AgentHookAcknowledgement: Codable, Equatable, Sendable {
    public let v: Int
    public let kind: String
    public let ok: Bool

    public init(
        v: Int = AgentHookProtocol.version,
        kind: String = AgentHookProtocol.acknowledgementKind,
        ok: Bool
    ) {
        self.v = v
        self.kind = kind
        self.ok = ok
    }
}

public enum AgentHookWireCodec {
    public static func encodeEventLine(_ message: AgentHookEventMessage) throws -> Data {
        try encodeLine(message)
    }

    public static func decodeEventLine(_ data: Data) throws -> AgentHookEventMessage {
        try JSONDecoder().decode(AgentHookEventMessage.self, from: linePayload(data))
    }

    public static func encodeAcknowledgementLine(_ acknowledgement: AgentHookAcknowledgement) throws -> Data {
        try encodeLine(acknowledgement)
    }

    public static func decodeAcknowledgementLine(_ data: Data) throws -> AgentHookAcknowledgement {
        try JSONDecoder().decode(AgentHookAcknowledgement.self, from: linePayload(data))
    }

    private static func encodeLine(_ value: some Encodable) throws -> Data {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        var data = try encoder.encode(value)
        data.append(UInt8(ascii: "\n"))
        return data
    }

    private static func linePayload(_ data: Data) -> Data {
        guard data.last == UInt8(ascii: "\n") else { return data }
        return data.dropLast()
    }
}
