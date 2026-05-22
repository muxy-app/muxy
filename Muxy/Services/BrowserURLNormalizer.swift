import Foundation

enum BrowserURLNormalizer {
    private static let recognizedSchemes: Set<String> = ["http", "https", "file", "about", "data"]

    static func normalize(_ input: String) -> URL? {
        let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }

        if let url = URL(string: trimmed),
           let scheme = url.scheme?.lowercased(),
           recognizedSchemes.contains(scheme)
        {
            return url
        }

        if looksLikeURL(trimmed) {
            if let url = URL(string: "http://\(trimmed)") {
                return url
            }
        }

        if let encoded = trimmed.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) {
            return URL(string: "https://duckduckgo.com/?q=\(encoded)")
        }

        return nil
    }

    private static func looksLikeURL(_ candidate: String) -> Bool {
        guard !candidate.contains(" ") else { return false }
        let lower = candidate.lowercased()
        if lower.hasPrefix("localhost") { return true }
        if lower.hasPrefix("127.0.0.1") { return true }
        if lower.hasPrefix("0.0.0.0") { return true }
        if lower.hasPrefix("[::1]") { return true }
        if lower.contains(".") { return true }
        if lower.contains(":"), lower.split(separator: ":").last?.allSatisfy(\.isNumber) == true {
            return true
        }
        return false
    }
}
