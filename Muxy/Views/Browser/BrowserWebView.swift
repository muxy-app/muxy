import AppKit
import os
import SwiftUI
import WebKit

private let browserLogger = Logger(subsystem: "app.muxy", category: "BrowserWebView")

final class BrowserWKWebView: WKWebView {
    var onReload: (() -> Void)?

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if event.modifierFlags.contains(.command),
           event.charactersIgnoringModifiers?.lowercased() == "r"
        {
            onReload?()
            return true
        }
        return super.performKeyEquivalent(with: event)
    }
}

struct BrowserWebView: NSViewRepresentable {
    let state: BrowserTabState

    func makeCoordinator() -> Coordinator {
        Coordinator(state: state)
    }

    func makeNSView(context: Context) -> BrowserWKWebView {
        let config = WKWebViewConfiguration()
        config.defaultWebpagePreferences.allowsContentJavaScript = true
        config.websiteDataStore = BrowserSession.dataStore

        let userContent = WKUserContentController()
        if let script = BrowserUserScripts.documentEndScript() {
            userContent.addUserScript(script)
        }
        if let css = BrowserUserScripts.documentStartCSS() {
            userContent.addUserScript(css)
        }
        userContent.add(context.coordinator.bridge, name: BrowserBridge.messageName)
        config.userContentController = userContent

        let webView = BrowserWKWebView(frame: .zero, configuration: config)
        webView.allowsBackForwardNavigationGestures = true
        webView.navigationDelegate = context.coordinator
        webView.uiDelegate = context.coordinator
        webView.setValue(false, forKey: "drawsBackground")

        context.coordinator.webView = webView
        context.coordinator.bridge.webView = webView
        context.coordinator.installObservers()

        webView.onReload = { [weak webView] in
            webView?.reload()
        }

        if let url = state.resolvedURL ?? URL(string: state.currentURL) {
            webView.load(URLRequest(url: url))
        }

        return webView
    }

    func updateNSView(_ webView: BrowserWKWebView, context: Context) {
        context.coordinator.sync(state: state, into: webView)
    }

    static func dismantleNSView(_ webView: BrowserWKWebView, coordinator: Coordinator) {
        coordinator.tearDown(webView: webView)
    }

    @MainActor
    final class Coordinator: NSObject, WKNavigationDelegate, WKUIDelegate {
        weak var webView: BrowserWKWebView?
        let bridge: BrowserBridge

        private let state: BrowserTabState
        private var lastNavigationVersion: Int = 0
        private var lastReloadVersion: Int = 0
        private var lastBackVersion: Int = 0
        private var lastForwardVersion: Int = 0
        private var lastZoomVersion: Int = 0
        private var lastScrollRestoreVersion: Int = 0
        private var lastInspectorModeVersion: Int = 0
        private var lastStyleOverridesVersion: Int = 0
        private var loadingObservation: NSKeyValueObservation?
        private var progressObservation: NSKeyValueObservation?
        private var canGoBackObservation: NSKeyValueObservation?
        private var canGoForwardObservation: NSKeyValueObservation?
        private var titleObservation: NSKeyValueObservation?
        private var urlObservation: NSKeyValueObservation?

        init(state: BrowserTabState) {
            self.state = state
            bridge = BrowserBridge(state: state, projectPath: state.projectPath)
        }

        func installObservers() {
            guard let webView else { return }
            loadingObservation = webView.observe(\.isLoading, options: [.new]) { [weak self] _, _ in
                Task { @MainActor in
                    guard let self else { return }
                    self.state.isLoading = webView.isLoading
                }
            }
            progressObservation = webView.observe(\.estimatedProgress, options: [.new]) { [weak self] _, _ in
                Task { @MainActor in
                    guard let self else { return }
                    self.state.estimatedProgress = webView.estimatedProgress
                }
            }
            canGoBackObservation = webView.observe(\.canGoBack, options: [.new]) { [weak self] _, _ in
                Task { @MainActor in
                    guard let self else { return }
                    self.state.canGoBack = webView.canGoBack
                }
            }
            canGoForwardObservation = webView.observe(\.canGoForward, options: [.new]) { [weak self] _, _ in
                Task { @MainActor in
                    guard let self else { return }
                    self.state.canGoForward = webView.canGoForward
                }
            }
            titleObservation = webView.observe(\.title, options: [.new]) { [weak self] _, _ in
                Task { @MainActor in
                    guard let self, let title = webView.title else { return }
                    self.state.pageTitle = title
                }
            }
            urlObservation = webView.observe(\.url, options: [.new]) { [weak self] _, _ in
                Task { @MainActor in
                    guard let self, let url = webView.url else { return }
                    self.state.currentURL = url.absoluteString
                    self.state.pendingURL = url.absoluteString
                }
            }
        }

        func tearDown(webView: BrowserWKWebView) {
            webView.configuration.userContentController.removeScriptMessageHandler(forName: BrowserBridge.messageName)
            loadingObservation = nil
            progressObservation = nil
            canGoBackObservation = nil
            canGoForwardObservation = nil
            titleObservation = nil
            urlObservation = nil
        }

        func sync(state: BrowserTabState, into webView: BrowserWKWebView) {
            if state.navigationRequestVersion != lastNavigationVersion {
                lastNavigationVersion = state.navigationRequestVersion
                if let url = state.resolvedURL {
                    webView.load(URLRequest(url: url))
                }
            }
            if state.reloadRequestVersion != lastReloadVersion {
                lastReloadVersion = state.reloadRequestVersion
                webView.reload()
            }
            if state.backRequestVersion != lastBackVersion {
                lastBackVersion = state.backRequestVersion
                webView.goBack()
            }
            if state.forwardRequestVersion != lastForwardVersion {
                lastForwardVersion = state.forwardRequestVersion
                webView.goForward()
            }
            if state.zoomRequestVersion != lastZoomVersion {
                lastZoomVersion = state.zoomRequestVersion
                webView.pageZoom = state.zoom
            }
            if state.scrollRestoreRequestVersion != lastScrollRestoreVersion {
                lastScrollRestoreVersion = state.scrollRestoreRequestVersion
                if let y = state.pendingScrollRestore {
                    webView.evaluateJavaScript("window.__muxyBrowserAPI && window.__muxyBrowserAPI.scrollTo(\(y));")
                }
            }
            if state.inspectorModeVersion != lastInspectorModeVersion {
                lastInspectorModeVersion = state.inspectorModeVersion
                webView.evaluateJavaScript(
                    "window.__muxyBrowserAPI && window.__muxyBrowserAPI.setMode('\(state.inspectorMode.rawValue)');"
                )
            }
            if state.styleOverridesVersion != lastStyleOverridesVersion {
                lastStyleOverridesVersion = state.styleOverridesVersion
                pushStyleOverrides(state: state, into: webView)
            }
        }

        private func pushStyleOverrides(state: BrowserTabState, into webView: WKWebView) {
            let overrides = state.aggregatedStyleOverrides()
            let grouped = Dictionary(grouping: overrides, by: \.selector)
            let rulesPayload = grouped.map { selector, items -> [String: Any] in
                [
                    "selector": selector,
                    "declarations": items.map { ["property": $0.property.cssName, "value": $0.value] },
                ]
            }
            guard let data = try? JSONSerialization.data(withJSONObject: rulesPayload),
                  let json = String(data: data, encoding: .utf8)
            else { return }
            webView.evaluateJavaScript(
                "window.__muxyBrowserAPI && window.__muxyBrowserAPI.applyOverrides(\(json));"
            )
        }

        func webView(_ webView: WKWebView, didFinish _: WKNavigation!) {
            Task { @MainActor in
                let url = webView.url?.absoluteString ?? state.currentURL
                let title = webView.title ?? ""
                self.state.handleNavigationFinished(
                    url: url,
                    title: title,
                    canGoBack: webView.canGoBack,
                    canGoForward: webView.canGoForward
                )
                if let pending = self.state.pendingScrollRestore {
                    webView.evaluateJavaScript("window.scrollTo(0, \(pending));")
                    self.state.pendingScrollRestore = nil
                }
                if self.state.inspectorMode != .off {
                    webView.evaluateJavaScript(
                        "window.__muxyBrowserAPI && window.__muxyBrowserAPI.setMode('\(self.state.inspectorMode.rawValue)');"
                    )
                }
                let overrides = self.state.aggregatedStyleOverrides()
                if !overrides.isEmpty {
                    self.pushStyleOverrides(state: self.state, into: webView)
                }
            }
        }

        func webView(_: WKWebView, didFail _: WKNavigation!, withError error: Error) {
            Task { @MainActor in
                self.state.handleNavigationFailed(message: error.localizedDescription)
            }
        }

        func webView(_: WKWebView, didFailProvisionalNavigation _: WKNavigation!, withError error: Error) {
            Task { @MainActor in
                self.state.handleNavigationFailed(message: error.localizedDescription)
            }
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
}

@MainActor
enum BrowserSession {
    static let dataStore: WKWebsiteDataStore = .default()
}

@MainActor
enum BrowserUserScripts {
    static func documentEndScript() -> WKUserScript? {
        guard let url = Bundle.appResources.url(forResource: "annotator", withExtension: "js"),
              let source = try? String(contentsOf: url, encoding: .utf8)
        else {
            browserLogger.error("Missing annotator.js resource")
            return nil
        }
        return WKUserScript(
            source: source,
            injectionTime: .atDocumentEnd,
            forMainFrameOnly: true
        )
    }

    static func documentStartCSS() -> WKUserScript? {
        guard let url = Bundle.appResources.url(forResource: "annotator", withExtension: "css"),
              let source = try? String(contentsOf: url, encoding: .utf8)
        else {
            return nil
        }
        let injected = """
        (function(){
          var s=document.createElement('style');
          s.id='muxy-browser-injected-style';
          s.setAttribute('data-muxy-overlay','');
          s.textContent=\(jsString(source));
          (document.head||document.documentElement).appendChild(s);
        })();
        """
        return WKUserScript(
            source: injected,
            injectionTime: .atDocumentStart,
            forMainFrameOnly: true
        )
    }

    private static func jsString(_ value: String) -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: [value], options: [.fragmentsAllowed]),
              var json = String(data: data, encoding: .utf8)
        else {
            return "\"\""
        }
        json = String(json.dropFirst().dropLast())
        return json
    }
}
