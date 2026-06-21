import Foundation

@MainActor
@Observable
final class BrowserTabState: Identifiable {
    enum NavigationCommand: Equatable {
        case back
        case forward
        case reload
        case stop
    }

    let id = UUID()
    let projectPath: String
    var profileID: UUID

    var url: URL?
    var pendingURL: URL?
    var pendingCommand: NavigationCommand?
    var pageTitle: String?
    var customTitle: String?
    var canGoBack: Bool = false
    var canGoForward: Bool = false
    var isLoading: Bool = false
    var estimatedProgress: Double = 0

    var displayTitle: String {
        if let customTitle, !customTitle.isEmpty { return customTitle }
        if let pageTitle, !pageTitle.isEmpty { return pageTitle }
        if let host = url?.host { return host }
        return "New Tab"
    }

    init(projectPath: String, url: URL? = nil, profileID: UUID = BrowserProfile.defaultID) {
        self.projectPath = projectPath
        self.url = url
        self.profileID = profileID
        pendingURL = url
    }

    func load(from input: String) {
        guard let resolved = BrowserURL.resolve(from: input) else { return }
        pendingURL = resolved
    }
}
