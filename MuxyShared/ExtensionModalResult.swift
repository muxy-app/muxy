import Foundation

public enum ExtensionModalResult {
    public static let messageHead = "modal-result"

    public struct Message: Equatable, Sendable {
        public let requestID: String
        public let payload: Data

        public init(requestID: String, payload: Data) {
            self.requestID = requestID
            self.payload = payload
        }
    }

    public static func serialize(requestID: String, payload: Data) -> String? {
        guard !requestID.isEmpty, !requestID.contains("|") else { return nil }
        return "\(messageHead)|\(requestID)|\(payload.base64EncodedString())"
    }

    public static func parse(_ line: String) -> Message? {
        let parts = line.split(separator: "|", maxSplits: 2, omittingEmptySubsequences: false).map(String.init)
        guard parts.count == 3, parts[0] == messageHead, !parts[1].isEmpty else { return nil }
        guard let payload = Data(base64Encoded: parts[2]) else { return nil }
        return Message(requestID: parts[1], payload: payload)
    }
}
