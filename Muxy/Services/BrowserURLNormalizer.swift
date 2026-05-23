import Foundation

enum BrowserURLNormalizer {
    static let webSchemes: Set<String> = ["http", "https"]
    static let aboutBlank = "about:blank"

    static func normalize(_ input: String) -> URL? {
        let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }

        if trimmed.lowercased() == aboutBlank, let url = URL(string: aboutBlank) {
            return url
        }

        if let url = URL(string: trimmed),
           let scheme = url.scheme?.lowercased()
        {
            if webSchemes.contains(scheme) { return url }
            if isBlockedScheme(scheme) { return nil }
        }

        if looksLikeURL(trimmed),
           let url = URL(string: "http://\(trimmed)")
        {
            return url
        }

        guard let encoded = trimmed.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) else {
            return nil
        }
        return URL(string: "https://www.google.com/search?q=\(encoded)")
    }

    static func isAllowedNavigationURL(_ url: URL) -> Bool {
        if url.absoluteString.lowercased() == aboutBlank { return true }
        guard let scheme = url.scheme?.lowercased() else { return false }
        return webSchemes.contains(scheme)
    }

    static func canonical(_ urlString: String) -> String {
        let trimmed = urlString.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return "" }
        guard var components = URLComponents(string: trimmed) else { return trimmed.lowercased() }
        components.scheme = components.scheme?.lowercased()
        components.host = components.host?.lowercased()
        components.fragment = nil
        if components.path.count > 1, components.path.hasSuffix("/") {
            components.path.removeLast()
        }
        if components.path.isEmpty, components.host != nil {
            components.path = "/"
        }
        return components.string ?? trimmed.lowercased()
    }

    private static func isBlockedScheme(_ scheme: String) -> Bool {
        scheme == "javascript" || scheme == "data" || scheme == "file"
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
