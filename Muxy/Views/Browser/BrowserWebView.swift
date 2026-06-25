import AppKit
import SwiftUI
import WebKit

struct BrowserWebView: NSViewRepresentable {
    let state: BrowserTabState
    let focused: Bool
    let overlayActive: Bool
    let appState: AppState
    let historyStore: BrowserHistoryStore

    func makeCoordinator() -> Coordinator {
        Coordinator(state: state, appState: appState, historyStore: historyStore)
    }

    func makeNSView(context: Context) -> WKWebView {
        let dataStore = BrowserDataStoreCache.shared.store(for: state.profileID)
        if let webView = Self.reusableWebView(for: state, dataStore: dataStore) {
            webView.navigationDelegate = context.coordinator
            webView.uiDelegate = context.coordinator
            context.coordinator.attach(to: webView)
            BrowserWebViewRegistry.shared.register(webView, for: state.id)
            webView.pageZoom = state.pageZoom
            return webView
        }

        let config = WKWebViewConfiguration()
        config.defaultWebpagePreferences.allowsContentJavaScript = true
        config.websiteDataStore = dataStore
        BrowserInspectableWebView.enableInspection(in: config)

        let webView = BrowserInspectableWebView(frame: .zero, configuration: config)
        webView.navigationDelegate = context.coordinator
        webView.uiDelegate = context.coordinator
        webView.isInspectable = true
        webView.allowsBackForwardNavigationGestures = true
        webView.allowsMagnification = true

        context.coordinator.attach(to: webView)
        state.webView = webView
        BrowserWebViewRegistry.shared.register(webView, for: state.id)
        webView.pageZoom = state.pageZoom
        if let url = state.navigationURLForWebViewMount() {
            webView.load(URLRequest(url: url))
        }
        return webView
    }

    func updateNSView(_ webView: WKWebView, context: Context) {
        context.coordinator.applyPendingCommand(in: webView)
        context.coordinator.applyPendingNavigation(in: webView)
        context.coordinator.applyPendingFind(in: webView)
        context.coordinator.applyFocusIfChanged(focused, overlayActive: overlayActive, in: webView)
    }

    static func dismantleNSView(_ webView: WKWebView, coordinator: Coordinator) {
        _ = (webView as? any BrowserElementInspecting)?.closeInspector()
        BrowserWebViewRegistry.shared.unregister(coordinator.tabID, ifMatches: webView)
        coordinator.detach(from: webView)
    }

    static func reusableWebView(for state: BrowserTabState, dataStore: WKWebsiteDataStore) -> WKWebView? {
        guard let webView = state.webView else { return nil }
        guard webView.configuration.websiteDataStore === dataStore else {
            retireCachedWebView(webView, state: state)
            return nil
        }
        return webView
    }

    private static func retireCachedWebView(_ webView: WKWebView, state: BrowserTabState) {
        _ = (webView as? any BrowserElementInspecting)?.closeInspector()
        webView.stopLoading()
        BrowserWebViewRegistry.shared.unregister(state.id, ifMatches: webView)
        if state.webView === webView {
            state.webView = nil
        }
    }

    @MainActor
    final class Coordinator: NSObject {
        private let state: BrowserTabState
        private let appState: AppState
        private let historyStore: BrowserHistoryStore
        private var observations: [NSKeyValueObservation] = []
        private var focused = false
        private var overlayActive = false

        var tabID: UUID { state.id }

        init(state: BrowserTabState, appState: AppState, historyStore: BrowserHistoryStore) {
            self.state = state
            self.appState = appState
            self.historyStore = historyStore
        }

        func attach(to webView: WKWebView) {
            observations = [
                webView.observe(\.estimatedProgress, options: [.new]) { [weak self] view, _ in
                    MainActor.assumeIsolated { self?.state.estimatedProgress = view.estimatedProgress }
                },
                webView.observe(\.isLoading, options: [.new]) { [weak self] view, _ in
                    MainActor.assumeIsolated { self?.state.isLoading = view.isLoading }
                },
                webView.observe(\.canGoBack, options: [.new]) { [weak self] view, _ in
                    MainActor.assumeIsolated { self?.state.canGoBack = view.canGoBack }
                },
                webView.observe(\.canGoForward, options: [.new]) { [weak self] view, _ in
                    MainActor.assumeIsolated { self?.state.canGoForward = view.canGoForward }
                },
                webView.observe(\.title, options: [.new]) { [weak self] view, _ in
                    MainActor.assumeIsolated { self?.handleTitleChange(view.title, url: view.url) }
                },
                webView.observe(\.url, options: [.new]) { [weak self] view, _ in
                    MainActor.assumeIsolated { self?.handleURLChange(view.url, title: view.title) }
                },
            ]
        }

        private func handleURLChange(_ url: URL?, title: String?) {
            state.url = url
            guard let url else { return }
            state.faviconImage = FaviconStore.shared.favicon(for: url)
            historyStore.record(url: url, title: title, profileID: state.profileID)
        }

        func extractFavicon(from webView: WKWebView) {
            guard let pageURL = webView.url else { return }
            let script = """
            (function() {
              var links = document.querySelectorAll('link[rel~="icon"]');
              if (links.length) { return links[links.length - 1].href; }
              return location.origin + '/favicon.ico';
            })()
            """
            webView.evaluateJavaScript(script) { [weak self] result, _ in
                MainActor.assumeIsolated {
                    guard let self,
                          let href = result as? String,
                          let iconURL = URL(string: href)
                    else { return }
                    self.state.faviconURL = iconURL
                    FaviconStore.shared.load(for: pageURL, iconURL: iconURL) { [weak self] image in
                        guard let image else { return }
                        self?.state.faviconImage = image
                    }
                }
            }
        }

        private func handleTitleChange(_ title: String?, url: URL?) {
            state.pageTitle = title
            guard let url else { return }
            historyStore.updateTitle(title, for: url, profileID: state.profileID)
        }

        func detach(from webView: WKWebView) {
            observations.forEach { $0.invalidate() }
            observations.removeAll()
            webView.navigationDelegate = nil
            webView.uiDelegate = nil
        }

        func applyPendingNavigation(in webView: WKWebView) {
            guard let url = state.consumePendingNavigationURL() else { return }
            webView.load(URLRequest(url: url))
        }

        func applyPendingCommand(in webView: WKWebView) {
            guard let command = state.pendingCommand else { return }
            state.pendingCommand = nil
            switch command {
            case .back: webView.goBack()
            case .forward: webView.goForward()
            case .reload: webView.reload()
            case .stop: webView.stopLoading()
            case .zoomIn: applyZoom(BrowserZoom.zoomIn(state.pageZoom), to: webView)
            case .zoomOut: applyZoom(BrowserZoom.zoomOut(state.pageZoom), to: webView)
            case .zoomReset: applyZoom(BrowserZoom.defaultValue, to: webView)
            case .inspectElement:
                _ = (webView as? BrowserInspectableWebView)?.inspectElement()
            }
        }

        private func applyZoom(_ zoom: Double, to webView: WKWebView) {
            state.pageZoom = zoom
            webView.pageZoom = zoom
        }

        func applyPendingFind(in webView: WKWebView) {
            guard let request = state.pendingFind else { return }
            state.pendingFind = nil
            guard !request.query.isEmpty else {
                state.findFoundMatch = true
                return
            }
            let configuration = WKFindConfiguration()
            configuration.backwards = request.backwards
            configuration.wraps = true
            webView.find(request.query, configuration: configuration) { [weak self] result in
                MainActor.assumeIsolated { self?.state.findFoundMatch = result.matchFound }
            }
        }

        func applyFocusIfChanged(_ focused: Bool, overlayActive: Bool, in webView: WKWebView) {
            guard focused != self.focused || overlayActive != self.overlayActive else { return }
            self.focused = focused
            self.overlayActive = overlayActive
            updateFirstResponder(for: webView)
        }

        private func updateFirstResponder(for webView: WKWebView) {
            DispatchQueue.main.async { [weak webView] in
                guard let webView, let window = webView.window else { return }
                if self.focused, !self.overlayActive {
                    window.makeFirstResponder(webView)
                } else if window.firstResponder === webView {
                    window.makeFirstResponder(nil)
                }
            }
        }
    }
}

extension BrowserWebView.Coordinator: WKNavigationDelegate, WKUIDelegate {
    @objc(_webView:getContextMenuFromProposedMenu:forElement:userInfo:completionHandler:)
    func webView(
        _ webView: WKWebView,
        getContextMenuFromProposedMenu menu: NSMenu,
        forElement _: Any,
        userInfo _: Any,
        completionHandler: @escaping (NSMenu) -> Void
    ) {
        (webView as? BrowserInspectableWebView)?.addInspectElementItem(to: menu)
        completionHandler(menu)
    }

    func webView(_: WKWebView, didStartProvisionalNavigation _: WKNavigation!) {
        state.loadError = nil
    }

    func webView(_ webView: WKWebView, didFinish _: WKNavigation!) {
        state.loadError = nil
        extractFavicon(from: webView)
    }

    func webView(_ webView: WKWebView, didFail _: WKNavigation!, withError error: Error) {
        state.loadError = BrowserLoadError.make(from: error, url: webView.url ?? state.url)
    }

    func webView(
        _ webView: WKWebView,
        didFailProvisionalNavigation _: WKNavigation!,
        withError error: Error
    ) {
        state.loadError = BrowserLoadError.make(from: error, url: webView.url ?? state.url)
    }

    func webView(
        _: WKWebView,
        decidePolicyFor navigationAction: WKNavigationAction,
        decisionHandler: @escaping @MainActor @Sendable (WKNavigationActionPolicy) -> Void
    ) {
        guard let url = navigationAction.request.url else {
            decisionHandler(.allow)
            return
        }
        if BrowserURL.isAllowed(url) {
            decisionHandler(.allow)
            return
        }
        decisionHandler(.cancel)
        guard navigationAction.navigationType == .linkActivated, isHandoffScheme(url) else { return }
        NSWorkspace.shared.open(url)
    }

    func webView(
        _: WKWebView,
        createWebViewWith _: WKWebViewConfiguration,
        for navigationAction: WKNavigationAction,
        windowFeatures _: WKWindowFeatures
    ) -> WKWebView? {
        if let url = navigationAction.request.url, BrowserURL.isAllowed(url) {
            appState.openInBuiltInBrowser(url, profileID: state.profileID)
        }
        return nil
    }

    private func isHandoffScheme(_ url: URL) -> Bool {
        guard let scheme = url.scheme?.lowercased() else { return false }
        return !["file", "javascript", "data"].contains(scheme)
    }
}
