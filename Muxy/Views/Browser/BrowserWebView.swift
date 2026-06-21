import SwiftUI
import WebKit

struct BrowserWebView: NSViewRepresentable {
    let state: BrowserTabState
    let focused: Bool
    let onFocus: () -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(state: state, onFocus: onFocus)
    }

    func makeNSView(context: Context) -> WKWebView {
        let config = WKWebViewConfiguration()
        config.defaultWebpagePreferences.allowsContentJavaScript = true

        let webView = WKWebView(frame: .zero, configuration: config)
        webView.navigationDelegate = context.coordinator
        webView.uiDelegate = context.coordinator
        webView.allowsBackForwardNavigationGestures = true
        webView.allowsMagnification = true

        context.coordinator.attach(to: webView)
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
        coordinator.detach(from: webView)
    }

    @MainActor
    final class Coordinator: NSObject {
        private let state: BrowserTabState
        private let onFocus: () -> Void
        private var observations: [NSKeyValueObservation] = []
        private var focused = false

        init(state: BrowserTabState, onFocus: @escaping () -> Void) {
            self.state = state
            self.onFocus = onFocus
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
            guard focused != self.focused else { return }
            self.focused = focused
            guard focused else { return }
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
        let scheme = url.scheme?.lowercased()
        if scheme == "http" || scheme == "https" || scheme == "about" {
            decisionHandler(.allow)
            return
        }
        NSWorkspace.shared.open(url)
        decisionHandler(.cancel)
    }

    func webView(
        _ webView: WKWebView,
        createWebViewWith _: WKWebViewConfiguration,
        for navigationAction: WKNavigationAction,
        windowFeatures _: WKWindowFeatures
    ) -> WKWebView? {
        if let url = navigationAction.request.url {
            webView.load(URLRequest(url: url))
        }
        return nil
    }
}
