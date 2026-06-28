import Foundation

struct ExtensionShortcut: Codable, Identifiable, Equatable {
    enum Source: String, Codable {
        case manifest
        case runtime
    }

    let extensionID: String
    let commandID: String
    var combo: KeyCombo
    var source: Source

    var id: String { "\(extensionID):\(commandID)" }

    var eventName: String { Self.eventName(forCommandID: commandID) }

    static func eventName(forCommandID commandID: String) -> String {
        "command.\(commandID)"
    }

    init(extensionID: String, commandID: String, combo: KeyCombo, source: Source = .manifest) {
        self.extensionID = extensionID
        self.commandID = commandID
        self.combo = combo
        self.source = source
    }

    private enum CodingKeys: String, CodingKey {
        case extensionID
        case commandID
        case combo
        case source
    }

    init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        extensionID = try container.decode(String.self, forKey: .extensionID)
        commandID = try container.decode(String.self, forKey: .commandID)
        combo = try container.decode(KeyCombo.self, forKey: .combo)
        source = try container.decodeIfPresent(Source.self, forKey: .source) ?? .manifest
    }
}
