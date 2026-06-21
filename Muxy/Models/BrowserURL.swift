import Foundation

enum BrowserURL {
    static let defaultURLString = "https://www.google.com"
    static let allowedSchemes: Set<String> = ["http", "https", "about"]

    static func isAllowed(_ url: URL) -> Bool {
        guard let scheme = url.scheme?.lowercased() else { return false }
        return allowedSchemes.contains(scheme)
    }

    static func resolve(from input: String) -> URL? {
        let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }

        if let url = URL(string: trimmed), isAllowed(url) {
            return url
        }

        if looksLikeHost(trimmed), let url = URL(string: "https://\(trimmed)"), isAllowed(url) {
            return url
        }

        return searchURL(for: trimmed)
    }

    static func searchURL(for query: String) -> URL? {
        var components = URLComponents(string: "https://www.google.com/search")
        components?.queryItems = [URLQueryItem(name: "q", value: query)]
        return components?.url
    }

    private static func looksLikeHost(_ value: String) -> Bool {
        guard !value.contains(" ") else { return false }
        if value == "localhost" || value.hasPrefix("localhost:") || value.hasPrefix("localhost/") { return true }
        let head = value.split(separator: "/", maxSplits: 1).first.map(String.init) ?? value
        let hostOnly = head.split(separator: ":", maxSplits: 1).first.map(String.init) ?? head
        return hostOnly.contains(".") && !hostOnly.hasPrefix(".") && !hostOnly.hasSuffix(".")
    }
}
