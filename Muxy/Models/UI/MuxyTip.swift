import Foundation

struct MuxyTip: Decodable, Equatable, Sendable {
    let description: String

    init(description: String) throws {
        let normalizedDescription = description.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalizedDescription.isEmpty else {
            throw DecodingError.dataCorrupted(.init(
                codingPath: [],
                debugDescription: "Tip descriptions cannot be empty"
            ))
        }
        self.description = normalizedDescription
    }

    init(from decoder: any Decoder) throws {
        let fields = try decoder.container(keyedBy: FieldKey.self)
        guard Set(fields.allKeys.map(\.stringValue)) == [CodingKeys.description.rawValue] else {
            throw DecodingError.dataCorrupted(.init(
                codingPath: decoder.codingPath,
                debugDescription: "Tips must contain exactly one description field"
            ))
        }
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(description: container.decode(String.self, forKey: .description))
    }

    private enum CodingKeys: String, CodingKey {
        case description
    }

    private struct FieldKey: CodingKey {
        let stringValue: String
        let intValue: Int?

        init?(stringValue: String) {
            self.stringValue = stringValue
            intValue = nil
        }

        init?(intValue: Int) {
            stringValue = String(intValue)
            self.intValue = intValue
        }
    }
}
