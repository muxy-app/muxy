import Foundation

public struct ExtensionModalQueryChange {
    public let requestID: String
    public let query: String

    public init(requestID: String, query: String) {
        self.requestID = requestID
        self.query = query
    }

    public static func parse(_ line: String) -> ExtensionModalQueryChange? {
        let parts = line.split(separator: "|", maxSplits: 2, omittingEmptySubsequences: false).map(String.init)
        guard parts.count >= 3, parts[0] == "modal-query-change" else { return nil }
        let requestID = parts[1]
        let query = parts[2].replacingOccurrences(of: "\\|", with: "|")
        guard !requestID.isEmpty else { return nil }
        return ExtensionModalQueryChange(requestID: requestID, query: query)
    }

    public func serialize() -> String {
        let escapedQuery = query.replacingOccurrences(of: "|", with: "\\|")
        return "modal-query-change|\(requestID)|\(escapedQuery)"
    }
}
