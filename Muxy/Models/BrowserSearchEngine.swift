import Foundation

enum BrowserSearchEngine: String, CaseIterable, Identifiable {
    case google
    case duckDuckGo
    case bing
    case brave
    case startpage

    var id: String { rawValue }

    var displayName: String {
        switch self {
        case .google: "Google"
        case .duckDuckGo: "DuckDuckGo"
        case .bing: "Bing"
        case .brave: "Brave"
        case .startpage: "Startpage"
        }
    }

    var endpoint: String {
        switch self {
        case .google: "https://www.google.com/search"
        case .duckDuckGo: "https://duckduckgo.com/"
        case .bing: "https://www.bing.com/search"
        case .brave: "https://search.brave.com/search"
        case .startpage: "https://www.startpage.com/sp/search"
        }
    }

    func searchURL(for query: String) -> URL? {
        var components = URLComponents(string: endpoint)
        components?.queryItems = [URLQueryItem(name: "q", value: query)]
        return components?.url
    }
}
