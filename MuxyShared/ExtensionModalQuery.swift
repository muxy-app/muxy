import Foundation

public enum ExtensionModalQuery {
    public static let messageHead = "modal-query"

    public struct Message: Equatable, Sendable {
        public let requestID: String
        public let queryID: Int
        public let query: String

        public init(requestID: String, queryID: Int, query: String) {
            self.requestID = requestID
            self.queryID = queryID
            self.query = query
        }
    }

    public static func serialize(requestID: String, queryID: Int, query: String) -> String? {
        guard !requestID.isEmpty, !requestID.contains("|") else { return nil }
        let payload = Data(query.utf8).base64EncodedString()
        return "\(messageHead)|\(requestID)|\(queryID)|\(payload)"
    }

    public static func parse(_ line: String) -> Message? {
        let parts = line.split(separator: "|", maxSplits: 3, omittingEmptySubsequences: false).map(String.init)
        guard parts.count == 4, parts[0] == messageHead, !parts[1].isEmpty else { return nil }
        guard let queryID = Int(parts[2]) else { return nil }
        guard let data = Data(base64Encoded: parts[3]), let query = String(data: data, encoding: .utf8) else { return nil }
        return Message(requestID: parts[1], queryID: queryID, query: query)
    }
}
