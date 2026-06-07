import Foundation

public enum ExtensionLocalEvent {
    public static let namePrefix = "extension."
    public static let messageHead = "extension-event"
    public static let maxNameLength = 200
    public static let maxPayloadBytes = 64 * 1024

    public enum PayloadError: Error, Equatable {
        case invalid
        case tooLarge(Int)
    }

    public struct Message: Equatable, Sendable {
        public let name: String
        public let payload: Data

        public init(name: String, payload: Data) {
            self.name = name
            self.payload = payload
        }
    }

    private static let allowedNameScalars = CharacterSet(
        charactersIn: "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._:-"
    )

    public static func isLocalName(_ name: String) -> Bool {
        name.hasPrefix(namePrefix)
    }

    public static func isValidName(_ name: String) -> Bool {
        guard name.hasPrefix(namePrefix),
              name.count > namePrefix.count,
              name.count <= maxNameLength
        else { return false }

        return name.unicodeScalars.allSatisfy { allowedNameScalars.contains($0) }
    }

    public static func encodePayload(_ value: Any?) throws -> Data {
        guard let value, !(value is NSNull) else {
            return Data("null".utf8)
        }
        let data: Data
        do {
            data = try JSONSerialization.data(withJSONObject: value, options: [.fragmentsAllowed])
        } catch {
            throw PayloadError.invalid
        }
        guard data.count <= maxPayloadBytes else {
            throw PayloadError.tooLarge(maxPayloadBytes)
        }
        return data
    }

    public static func serialize(name: String, payload: Data) -> String? {
        guard isValidName(name), payload.count <= maxPayloadBytes else { return nil }
        let encodedName = Data(name.utf8).base64EncodedString()
        return "\(messageHead)|\(encodedName)|\(payload.base64EncodedString())"
    }

    public static func parse(_ line: String) -> Message? {
        let parts = line.split(separator: "|", maxSplits: 2, omittingEmptySubsequences: false).map(String.init)
        guard parts.count == 3, parts[0] == messageHead else { return nil }
        guard let nameData = Data(base64Encoded: parts[1]),
              let name = String(data: nameData, encoding: .utf8),
              isValidName(name),
              let payload = Data(base64Encoded: parts[2]),
              payload.count <= maxPayloadBytes
        else { return nil }
        return Message(name: name, payload: payload)
    }
}
