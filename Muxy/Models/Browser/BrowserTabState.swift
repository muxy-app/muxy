import AppKit
import Foundation
import WebKit

@MainActor
protocol BrowserTabSurfaceRuntime: AnyObject {
    func retire(webView: WKWebView?)
}

@MainActor
@Observable
final class BrowserTabState: Identifiable {
    enum NavigationCommand: Equatable {
        case back
        case forward
        case reload
        case stop
        case zoomIn
        case zoomOut
        case zoomReset
        case inspectElement
    }

    struct FindRequest: Equatable {
        let query: String
        let backwards: Bool
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
    var shouldFocusAddressOnOpen = true
    var pageZoom: Double = 1
    var loadError: BrowserLoadError?
    var faviconURL: URL?
    var faviconImage: NSImage?
    var pendingFind: FindRequest?
    var findActivationToken = 0
    var findFoundMatch = true
    @ObservationIgnored var surfaceRuntime: (any BrowserTabSurfaceRuntime)?
    @ObservationIgnored var webView: WKWebView? {
        didSet {
            guard oldValue !== webView else { return }
            if let oldValue {
                surfaceRuntime?.retire(webView: oldValue)
                surfaceRuntime = nil
                BrowserWebViewRegistry.shared.unregister(id, ifMatches: oldValue)
            }
            if let webView {
                BrowserWebViewRegistry.shared.register(webView, for: id)
            }
        }
    }

    var isBlank: Bool {
        guard let absoluteString = url?.absoluteString else { return true }
        return BrowserHomePage.isBlankMode(absoluteString)
    }

    var displayTitle: String {
        if let customTitle, !customTitle.isEmpty {
            return customTitle
        }
        if let pageTitle, !pageTitle.isEmpty {
            return pageTitle
        }
        if let host = url?.host {
            return host
        }
        return "New Tab"
    }

    init(projectPath: String, url: URL? = nil, profileID: UUID = BrowserProfile.defaultID) {
        self.projectPath = projectPath
        self.url = url
        self.profileID = profileID
        pendingURL = url
    }

    deinit {
        MainActor.assumeIsolated {
            surfaceRuntime?.retire(webView: webView)
            BrowserWebViewRegistry.shared.unregister(id)
        }
    }

    func load(from input: String) {
        guard let resolved = BrowserURL.resolve(from: input) else { return }
        navigate(to: resolved)
    }

    func navigate(to url: URL) {
        guard let webView else {
            pendingURL = url
            return
        }
        pendingURL = nil
        webView.load(URLRequest(url: url))
    }

    func switchProfile(to id: UUID) {
        guard id != profileID else { return }
        profileID = id
        guard let url else { return }
        pendingURL = url
    }

    func navigationURLForWebViewMount() -> URL? {
        if let pendingURL {
            self.pendingURL = nil
            url = pendingURL
            return pendingURL
        }
        guard let url, !BrowserHomePage.isBlankMode(url.absoluteString) else { return nil }
        return url
    }

    func consumePendingNavigationURL() -> URL? {
        guard let pendingURL else { return nil }
        self.pendingURL = nil
        url = pendingURL
        return pendingURL
    }

    func activateFind() {
        findActivationToken += 1
    }
}
