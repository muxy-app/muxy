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
    public let id: String?
    public let provider: String
    public let paneID: String?
    public let phase: AgentHookPhase
    public let title: String
    public let body: String
    public let pids: [Int32]
    public let ts: Int64
    public let test: Bool

    public init(
        v: Int = AgentHookProtocol.version,
        kind: String = AgentHookProtocol.eventKind,
        id: String? = nil,
        provider: String,
        paneID: String?,
        phase: AgentHookPhase,
        title: String,
        body: String,
        pids: [Int32],
        ts: Int64,
        test: Bool = false
    ) {
        self.v = v
        self.kind = kind
        self.id = id
        self.provider = provider
        self.paneID = paneID
        self.phase = phase
        self.title = title
        self.body = body
        self.pids = pids
        self.ts = ts
        self.test = test
    }

    private enum CodingKeys: String, CodingKey {
        case v
        case kind
        case id
        case provider
        case paneID
        case phase
        case title
        case body
        case pids
        case ts
        case test
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        v = try container.decode(Int.self, forKey: .v)
        kind = try container.decode(String.self, forKey: .kind)
        id = try container.decodeIfPresent(String.self, forKey: .id)
        provider = try container.decode(String.self, forKey: .provider)
        paneID = try container.decodeIfPresent(String.self, forKey: .paneID)
        phase = try container.decode(AgentHookPhase.self, forKey: .phase)
        title = try container.decode(String.self, forKey: .title)
        body = try container.decode(String.self, forKey: .body)
        pids = try container.decode([Int32].self, forKey: .pids)
        ts = try container.decode(Int64.self, forKey: .ts)
        test = try container.decodeIfPresent(Bool.self, forKey: .test) ?? false
    }

    public func encode(to encoder: any Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(v, forKey: .v)
        try container.encode(kind, forKey: .kind)
        try container.encodeIfPresent(id, forKey: .id)
        try container.encode(provider, forKey: .provider)
        try container.encodeIfPresent(paneID, forKey: .paneID)
        try container.encode(phase, forKey: .phase)
        try container.encode(title, forKey: .title)
        try container.encode(body, forKey: .body)
        try container.encode(pids, forKey: .pids)
        try container.encode(ts, forKey: .ts)
        if test {
            try container.encode(test, forKey: .test)
        }
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
