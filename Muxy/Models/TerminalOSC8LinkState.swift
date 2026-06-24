import Foundation

enum TerminalOpenURLParser {
    static func url(from raw: String) -> URL? {
        guard !raw.isEmpty else { return nil }
        if let url = URL(string: raw) { return url }
        guard let encoded = raw.addingPercentEncoding(withAllowedCharacters: .urlFragmentAllowed) else {
            return nil
        }
        return URL(string: encoded)
    }
}

struct TerminalOSC8LinkState: Equatable, Sendable {
    var hasLinkUnderCursor: Bool = false
    var stickyURL: URL?

    var shouldShowLinkCursor: Bool {
        hasLinkUnderCursor || stickyURL != nil
    }

    mutating func applyHover(urlString: String?, commandHeld: Bool) {
        if let urlString, !urlString.isEmpty, let url = URL(string: urlString) {
            stickyURL = url
            hasLinkUnderCursor = true
            return
        }

        hasLinkUnderCursor = false
        if !commandHeld {
            stickyURL = nil
        }
    }

    func urlToOpenOnCommandClick() -> URL? {
        stickyURL
    }
}
