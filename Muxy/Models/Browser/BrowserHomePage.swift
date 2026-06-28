import Foundation

enum BrowserHomePage {
    static let blankURLString = "about:blank"

    static var blankURL: URL? { URL(string: blankURLString) }

    static func isBlankMode(_ urlString: String) -> Bool {
        normalized(urlString) == blankURLString
    }

    static func normalized(_ urlString: String) -> String {
        let trimmed = urlString.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? blankURLString : trimmed
    }

    static func resolvedURL(from urlString: String) -> URL? {
        let normalizedString = normalized(urlString)
        guard normalizedString != blankURLString else { return blankURL }
        return BrowserURL.resolve(from: normalizedString)
    }
}
