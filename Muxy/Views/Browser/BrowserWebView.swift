import SwiftUI
import WebKit

struct BrowserWebView: NSViewRepresentable {
    let state: BrowserTabState
    let focused: Bool
    let appState: AppState

    func makeCoordinator() -> Coordinator {
        Coordinator(state: state, appState: appState)
    }

    func makeNSView(context: Context) -> WKWebView {
        let config = WKWebViewConfiguration()
        config.defaultWebpagePreferences.allowsContentJavaScript = true
        config.websiteDataStore = BrowserDataStoreCache.shared.store(for: state.profileID)

        let webView = WKWebView(frame: .zero, configuration: config)
        webView.navigationDelegate = context.coordinator
        webView.uiDelegate = context.coordinator
        webView.allowsBackForwardNavigationGestures = true
        webView.allowsMagnification = true

        context.coordinator.attach(to: webView)
        BrowserWebViewRegistry.shared.register(webView, for: state.id)
        if let url = state.pendingURL {
            state.pendingURL = nil
            webView.load(URLRequest(url: url))
        }
        return webView
    }

    func updateNSView(_ webView: WKWebView, context: Context) {
        context.coordinator.applyPendingCommand(in: webView)
        context.coordinator.applyPendingNavigation(in: webView)
        context.coordinator.applyFocusIfChanged(focused, in: webView)
    }

    static func dismantleNSView(_ webView: WKWebView, coordinator: Coordinator) {
        BrowserWebViewRegistry.shared.unregister(coordinator.tabID)
        coordinator.detach(from: webView)
    }

    @MainActor
    final class Coordinator: NSObject {
        private let state: BrowserTabState
        private let appState: AppState
        private var observations: [NSKeyValueObservation] = []
        private var focused = false

        var tabID: UUID { state.id }

        init(state: BrowserTabState, appState: AppState) {
            self.state = state
            self.appState = appState
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
                    MainActor.assumeIsolated { self?.state.pageTitle = view.title }
                },
                webView.observe(\.url, options: [.new]) { [weak self] view, _ in
                    MainActor.assumeIsolated { self?.state.url = view.url }
                },
            ]
        }

        func detach(from webView: WKWebView) {
            observations.forEach { $0.invalidate() }
            observations.removeAll()
            webView.navigationDelegate = nil
            webView.uiDelegate = nil
        }

        func applyPendingNavigation(in webView: WKWebView) {
            guard let url = state.pendingURL else { return }
            state.pendingURL = nil
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
            }
        }

        func applyFocusIfChanged(_ focused: Bool, in webView: WKWebView) {
            guard focused else {
                self.focused = false
                return
            }
            guard !self.focused, webView.window != nil else { return }
            self.focused = true
            DispatchQueue.main.async { [weak webView] in
                guard let webView, let window = webView.window else { return }
                window.makeFirstResponder(webView)
            }
        }
    }
}

extension BrowserWebView.Coordinator: WKNavigationDelegate, WKUIDelegate {
    func webView(
        _: WKWebView,
        decidePolicyFor navigationAction: WKNavigationAction,
        decisionHandler: @escaping (WKNavigationActionPolicy) -> Void
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
