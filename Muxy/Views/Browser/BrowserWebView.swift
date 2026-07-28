import AppKit
import SwiftUI
import WebKit

struct BrowserWebView: NSViewRepresentable {
    let state: BrowserTabState
    let focused: Bool
    let overlayActive: Bool
    let appState: AppState
    let historyStore: BrowserHistoryStore
    let topLevelGroupID: UUID
    @Environment(\.activeWorktreeKey) private var worktreeKey

    private static let mountBroker = ReparentingNSViewBroker<WKWebView> { webView in
        _ = (webView as? any BrowserElementInspecting)?.closeInspector()
    }

    func makeCoordinator() -> Coordinator {
        if let coordinator = state.surfaceRuntime as? Coordinator {
            coordinator.update(appState: appState, historyStore: historyStore)
            return coordinator
        }
        let coordinator = Coordinator(state: state, appState: appState, historyStore: historyStore)
        state.surfaceRuntime = coordinator
        return coordinator
    }

    func makeNSView(context: Context) -> ReparentingNSViewHost {
        let host = ReparentingNSViewHost()
        let webView = resolvedWebView()
        state.surfaceRuntime = context.coordinator
        Self.mountBroker.register(
            claimID: context.coordinator.claimID,
            view: webView,
            host: host,
            configuration: mountConfiguration(coordinator: context.coordinator)
        )
        return host
    }

    func updateNSView(_ host: ReparentingNSViewHost, context: Context) {
        let webView = resolvedWebView()
        state.surfaceRuntime = context.coordinator
        Self.mountBroker.update(
            claimID: context.coordinator.claimID,
            view: webView,
            host: host,
            configuration: mountConfiguration(coordinator: context.coordinator)
        )
    }

    static func dismantleNSView(_ host: ReparentingNSViewHost, coordinator: Coordinator) {
        guard mountBroker.release(claimID: coordinator.claimID, host: host) else { return }
        guard !coordinator.isRetainedByState else { return }
        coordinator.retire(webView: nil)
    }

    private var isCurrentPresentation: Bool {
        guard let worktreeKey,
              let visibleLayout = appState.visibleLayout(
                  for: worktreeKey,
                  groupID: topLevelGroupID
              )
        else { return false }
        return visibleLayout.allPanes().contains {
            $0.tab.content.browserState?.id == state.id
        }
    }

    private func mountConfiguration(
        coordinator: Coordinator
    ) -> ReparentingNSViewBroker<WKWebView>.Configuration {
        .init(
            isEligible: { isCurrentPresentation },
            prepare: { webView in
                coordinator.attach(to: webView)
                webView.pageZoom = state.pageZoom
                coordinator.applyPendingCommand(in: webView)
                coordinator.applyPendingNavigation(in: webView)
                coordinator.applyPendingFind(in: webView)
            },
            didMount: { webView, host, ownershipChanged in
                coordinator.applyFocusIfChanged(
                    focused,
                    overlayActive: overlayActive,
                    in: webView,
                    host: host,
                    reset: ownershipChanged
                )
            }
        )
    }

    private func resolvedWebView() -> WKWebView {
        let dataStore = BrowserDataStoreCache.shared.store(for: state.profileID)
        if let webView = Self.reusableWebView(for: state, dataStore: dataStore) {
            return webView
        }

        let config = WKWebViewConfiguration()
        config.defaultWebpagePreferences.allowsContentJavaScript = true
        config.websiteDataStore = dataStore
        BrowserInspectableWebView.enableInspection(in: config)

        let webView = BrowserInspectableWebView(frame: .zero, configuration: config)
        webView.isInspectable = true
        webView.allowsBackForwardNavigationGestures = true
        webView.allowsMagnification = true

        state.webView = webView
        if let url = state.navigationURLForWebViewMount() {
            webView.load(URLRequest(url: url))
        }
        return webView
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
        if state.webView === webView {
            state.webView = nil
        }
    }

    @MainActor
    final class Coordinator: NSObject, BrowserTabSurfaceRuntime {
        let claimID = UUID()
        private weak var state: BrowserTabState?
        private weak var appState: AppState?
        private var historyStore: BrowserHistoryStore
        private var observations: [NSKeyValueObservation] = []
        private weak var attachedWebView: WKWebView?
        private var focused = false
        private var overlayActive = false

        var activeObservationCount: Int {
            observations.count
        }

        var isRetainedByState: Bool {
            state?.surfaceRuntime === self
        }

        init(state: BrowserTabState, appState: AppState, historyStore: BrowserHistoryStore) {
            self.state = state
            self.appState = appState
            self.historyStore = historyStore
        }

        func update(appState: AppState, historyStore: BrowserHistoryStore) {
            self.appState = appState
            self.historyStore = historyStore
        }

        func attach(to webView: WKWebView) {
            if let displacedCoordinator = webView.navigationDelegate as? Coordinator,
               displacedCoordinator !== self
            {
                displacedCoordinator.detach()
            }
            if let displacedCoordinator = webView.uiDelegate as? Coordinator,
               displacedCoordinator !== self
            {
                displacedCoordinator.detach()
            }
            webView.navigationDelegate = self
            webView.uiDelegate = self
            guard attachedWebView !== webView else { return }
            detach()
            attachedWebView = webView
            observations = [
                webView.observe(\.estimatedProgress, options: [.new]) { [weak self] view, _ in
                    MainActor.assumeIsolated { self?.state?.estimatedProgress = view.estimatedProgress }
                },
                webView.observe(\.isLoading, options: [.new]) { [weak self] view, _ in
                    MainActor.assumeIsolated { self?.state?.isLoading = view.isLoading }
                },
                webView.observe(\.canGoBack, options: [.new]) { [weak self] view, _ in
                    MainActor.assumeIsolated { self?.state?.canGoBack = view.canGoBack }
                },
                webView.observe(\.canGoForward, options: [.new]) { [weak self] view, _ in
                    MainActor.assumeIsolated { self?.state?.canGoForward = view.canGoForward }
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
            guard let state else { return }
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
                    self.state?.faviconURL = iconURL
                    FaviconStore.shared.load(for: pageURL, iconURL: iconURL) { [weak self] image in
                        guard let image else { return }
                        self?.state?.faviconImage = image
                    }
                }
            }
        }

        private func handleTitleChange(_ title: String?, url: URL?) {
            guard let state else { return }
            state.pageTitle = title
            guard let url else { return }
            historyStore.updateTitle(title, for: url, profileID: state.profileID)
        }

        func detach() {
            observations.forEach { $0.invalidate() }
            observations.removeAll()
            attachedWebView = nil
        }

        func retire(webView: WKWebView?) {
            if let webView, attachedWebView !== webView {
                return
            }
            let retiredWebView = attachedWebView
            detach()
            if retiredWebView?.navigationDelegate === self {
                retiredWebView?.navigationDelegate = nil
            }
            if retiredWebView?.uiDelegate === self {
                retiredWebView?.uiDelegate = nil
            }
        }

        func applyPendingNavigation(in webView: WKWebView) {
            guard let url = state?.consumePendingNavigationURL() else { return }
            webView.load(URLRequest(url: url))
        }

        func applyPendingCommand(in webView: WKWebView) {
            guard let state, let command = state.pendingCommand else { return }
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
            state?.pageZoom = zoom
            webView.pageZoom = zoom
        }

        func applyPendingFind(in webView: WKWebView) {
            guard let state, let request = state.pendingFind else { return }
            state.pendingFind = nil
            guard !request.query.isEmpty else {
                state.findFoundMatch = true
                return
            }
            let configuration = WKFindConfiguration()
            configuration.backwards = request.backwards
            configuration.wraps = true
            webView.find(request.query, configuration: configuration) { [weak self] result in
                MainActor.assumeIsolated { self?.state?.findFoundMatch = result.matchFound }
            }
        }

        func applyFocusIfChanged(
            _ focused: Bool,
            overlayActive: Bool,
            in webView: WKWebView,
            host: ReparentingNSViewHost,
            reset: Bool
        ) {
            guard reset || focused != self.focused || overlayActive != self.overlayActive else { return }
            self.focused = focused
            self.overlayActive = overlayActive
            updateFirstResponder(for: webView, in: host)
        }

        private func updateFirstResponder(
            for webView: WKWebView,
            in host: ReparentingNSViewHost
        ) {
            DispatchQueue.main.async { [weak self, weak webView, weak host] in
                guard let self,
                      let webView,
                      let host,
                      webView.superview === host,
                      let window = webView.window
                else { return }
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
        state?.loadError = nil
    }

    func webView(_ webView: WKWebView, didFinish _: WKNavigation!) {
        state?.loadError = nil
        extractFavicon(from: webView)
    }

    func webView(_ webView: WKWebView, didFail _: WKNavigation!, withError error: Error) {
        state?.loadError = BrowserLoadError.make(from: error, url: webView.url ?? state?.url)
    }

    func webView(
        _ webView: WKWebView,
        didFailProvisionalNavigation _: WKNavigation!,
        withError error: Error
    ) {
        state?.loadError = BrowserLoadError.make(from: error, url: webView.url ?? state?.url)
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
        if let url = navigationAction.request.url,
           BrowserURL.isAllowed(url),
           let appState,
           let profileID = state?.profileID
        {
            appState.openInBuiltInBrowser(url, profileID: profileID)
        }
        return nil
    }

    private func isHandoffScheme(_ url: URL) -> Bool {
        guard let scheme = url.scheme?.lowercased() else { return false }
        return !["file", "javascript", "data"].contains(scheme)
    }
}
