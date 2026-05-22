import Foundation

@MainActor
@Observable
final class BrowserNavigationState {
    var pendingURL: String
    var currentURL: String
    var pageTitle: String = ""
    var faviconData: Data?

    var canGoBack: Bool = false
    var canGoForward: Bool = false
    var isLoading: Bool = false
    var estimatedProgress: Double = 0
    var lastErrorMessage: String?

    var zoom: Double = 1.0
    var scrollY: Double = 0
    var pendingScrollRestore: Double?

    init(initialURL: String, scrollY: Double, zoom: Double) {
        pendingURL = initialURL
        currentURL = initialURL
        self.scrollY = scrollY
        self.zoom = zoom
        pendingScrollRestore = scrollY > 0 ? scrollY : nil
    }

    var displayTitle: String {
        if !pageTitle.isEmpty { return pageTitle }
        if let host = URL(string: currentURL)?.host { return host }
        return "Browser"
    }

    var resolvedURL: URL? {
        BrowserURLNormalizer.normalize(pendingURL)
    }

    var currentURLScheme: String? {
        URL(string: currentURL)?.scheme?.lowercased()
    }

    func handleProgress(_ progress: Double, isLoading: Bool) {
        estimatedProgress = progress
        self.isLoading = isLoading
    }

    func handleNavigationFinished(url: String, title: String, canGoBack: Bool, canGoForward: Bool) {
        currentURL = url
        pendingURL = url
        pageTitle = title
        self.canGoBack = canGoBack
        self.canGoForward = canGoForward
        isLoading = false
        estimatedProgress = 0
        lastErrorMessage = nil
    }

    func handleNavigationFailed(message: String) {
        lastErrorMessage = message
        isLoading = false
        estimatedProgress = 0
    }
}
