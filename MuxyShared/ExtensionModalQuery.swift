import Foundation

public enum ExtensionModalQuery {
    public static let messageHead = "modal-query"

    public struct Message: Equatable, Sendable {
        public let requestID: String
        public let queryID: Int
        public let query: String
        public let options: [String: Bool]

        public init(requestID: String, queryID: Int, query: String, options: [String: Bool] = [:]) {
            self.requestID = requestID
            self.queryID = queryID
            self.query = query
            self.options = options
        }
    }

    public static func serialize(
        requestID: String,
        queryID: Int,
        query: String,
        options: [String: Bool] = [:]
    ) -> String? {
        guard !requestID.isEmpty, !requestID.contains("|") else { return nil }
        let payload = Data(query.utf8).base64EncodedString()
        guard !options.isEmpty else {
            return "\(messageHead)|\(requestID)|\(queryID)|\(payload)"
        }
        guard let optionsData = try? JSONSerialization.data(withJSONObject: options) else { return nil }
        let optionsPayload = optionsData.base64EncodedString()
        return "\(messageHead)|\(requestID)|\(queryID)|\(payload)|\(optionsPayload)"
    }

    public static func parse(_ line: String) -> Message? {
        let parts = line.split(separator: "|", maxSplits: 4, omittingEmptySubsequences: false).map(String.init)
        guard parts.count == 4 || parts.count == 5, parts[0] == messageHead, !parts[1].isEmpty else { return nil }
        guard let queryID = Int(parts[2]) else { return nil }
        guard let data = Data(base64Encoded: parts[3]), let query = String(data: data, encoding: .utf8) else { return nil }
        guard parts.count == 5 else {
            return Message(requestID: parts[1], queryID: queryID, query: query)
        }
        guard let optionsData = Data(base64Encoded: parts[4]),
              let options = try? JSONSerialization.jsonObject(with: optionsData) as? [String: Bool]
        else { return nil }
        return Message(requestID: parts[1], queryID: queryID, query: query, options: options)
    }
}
