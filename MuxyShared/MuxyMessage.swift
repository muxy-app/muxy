import Foundation

public enum MuxyMessage: Codable, Sendable {
    case request(MuxyRequest)
    case response(MuxyResponse)
    case event(MuxyEvent)

    private enum CodingKeys: String, CodingKey {
        case type
        case payload
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(MuxyMessageType.self, forKey: .type)
        switch type {
        case .request: self = try .request(container.decode(MuxyRequest.self, forKey: .payload))
        case .response: self = try .response(container.decode(MuxyResponse.self, forKey: .payload))
        case .event: self = try .event(container.decode(MuxyEvent.self, forKey: .payload))
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .request(r):
            try container.encode(MuxyMessageType.request, forKey: .type)
            try container.encode(r, forKey: .payload)
        case let .response(r):
            try container.encode(MuxyMessageType.response, forKey: .type)
            try container.encode(r, forKey: .payload)
        case let .event(e):
            try container.encode(MuxyMessageType.event, forKey: .type)
            try container.encode(e, forKey: .payload)
        }
    }
}

public enum MuxyCodec {
    private static let encoder: JSONEncoder = {
        let e = JSONEncoder()
        e.dateEncodingStrategy = .iso8601
        return e
    }()

    private static let decoder: JSONDecoder = {
        let d = JSONDecoder()
        d.dateDecodingStrategy = .iso8601
        return d
    }()

    public static func encode(_ message: MuxyMessage) throws -> Data {
        try encoder.encode(message)
    }

    public static func decode(_ data: Data) throws -> MuxyMessage {
        try decoder.decode(MuxyMessage.self, from: data)
    }
}
