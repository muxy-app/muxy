import AppKit
import os
import SwiftUI
import WebKit

private let browserLogger = Logger(subsystem: "app.muxy", category: "BrowserWebView")

final class BrowserWKWebView: WKWebView {
    var onReload: (() -> Void)?
    var onFindShortcut: (() -> Void)?

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if event.modifierFlags.contains(.command),
           let character = event.charactersIgnoringModifiers?.lowercased()
        {
            switch character {
            case "r":
                onReload?()
                return true
            case "f":
                onFindShortcut?()
                return true
            default:
                break
            }
        }
        return super.performKeyEquivalent(with: event)
    }
}

struct BrowserWebView: NSViewRepresentable {
    let session: BrowserSession

    func makeCoordinator() -> Coordinator {
        Coordinator(session: session)
    }

    func makeNSView(context: Context) -> BrowserWKWebView {
        let config = WKWebViewConfiguration()
        config.defaultWebpagePreferences.allowsContentJavaScript = true
        config.websiteDataStore = BrowserDataStoreFactory.dataStore()

        let userContent = WKUserContentController()
        if let script = BrowserUserScripts.documentEndScript() {
            userContent.addUserScript(script)
        }
        if let css = BrowserUserScripts.documentStartCSS() {
            userContent.addUserScript(css)
        }
        userContent.add(
            context.coordinator.bridge,
            contentWorld: BrowserBridge.contentWorld,
            name: BrowserBridge.messageName
        )
        config.userContentController = userContent

        let webView = BrowserWKWebView(frame: .zero, configuration: config)
        webView.allowsBackForwardNavigationGestures = true
        webView.allowsLinkPreview = false
        webView.navigationDelegate = context.coordinator
        webView.uiDelegate = context.coordinator
        webView.setValue(false, forKey: "drawsBackground")
        if #available(macOS 13.3, *) {
            webView.isInspectable = BrowserPreferences.inspectable
        }

        context.coordinator.webView = webView
        context.coordinator.bridge.webView = webView
        context.coordinator.installObservers()
        context.coordinator.startConsumingCommands()

        webView.onReload = { [weak webView] in
            webView?.reload()
        }
        webView.onFindShortcut = { [session] in
            session.presentFindBar()
        }

        if let url = session.nav.resolvedURL ?? URL(string: session.nav.currentURL),
           BrowserURLNormalizer.isAllowedNavigationURL(url)
        {
            webView.load(URLRequest(url: url))
        }

        return webView
    }

    func updateNSView(_: BrowserWKWebView, context _: Context) {}

    static func dismantleNSView(_ webView: BrowserWKWebView, coordinator: Coordinator) {
        coordinator.tearDown(webView: webView)
    }

    @MainActor
    final class Coordinator: NSObject, WKNavigationDelegate, WKUIDelegate {
        weak var webView: BrowserWKWebView?
        let bridge: BrowserBridge

        private let session: BrowserSession
        private var commandTask: Task<Void, Never>?
        private var loadingObservation: NSKeyValueObservation?
        private var progressObservation: NSKeyValueObservation?
        private var canGoBackObservation: NSKeyValueObservation?
        private var canGoForwardObservation: NSKeyValueObservation?
        private var titleObservation: NSKeyValueObservation?
        private var urlObservation: NSKeyValueObservation?

        init(session: BrowserSession) {
            self.session = session
            bridge = BrowserBridge(session: session)
        }

        func installObservers() {
            guard let webView else { return }
            loadingObservation = webView.observe(\.isLoading, options: [.new]) { [weak self] _, _ in
                Task { @MainActor in
                    guard let self else { return }
                    self.session.nav.isLoading = webView.isLoading
                }
            }
            progressObservation = webView.observe(\.estimatedProgress, options: [.new]) { [weak self] _, _ in
                Task { @MainActor in
                    guard let self else { return }
                    self.session.nav.estimatedProgress = webView.estimatedProgress
                }
            }
            canGoBackObservation = webView.observe(\.canGoBack, options: [.new]) { [weak self] _, _ in
                Task { @MainActor in
                    guard let self else { return }
                    self.session.nav.canGoBack = webView.canGoBack
                }
            }
            canGoForwardObservation = webView.observe(\.canGoForward, options: [.new]) { [weak self] _, _ in
                Task { @MainActor in
                    guard let self else { return }
                    self.session.nav.canGoForward = webView.canGoForward
                }
            }
            titleObservation = webView.observe(\.title, options: [.new]) { [weak self] _, _ in
                Task { @MainActor in
                    guard let self, let title = webView.title else { return }
                    self.session.nav.pageTitle = title
                }
            }
            urlObservation = webView.observe(\.url, options: [.new]) { [weak self] _, _ in
                Task { @MainActor in
                    guard let self, let url = webView.url else { return }
                    self.session.nav.currentURL = url.absoluteString
                    self.session.nav.pendingURL = url.absoluteString
                }
            }
        }

        func startConsumingCommands() {
            commandTask?.cancel()
            let stream = session.commands.stream
            commandTask = Task { @MainActor [weak self] in
                for await command in stream {
                    guard let self else { return }
                    self.handle(command: command)
                }
            }
        }

        func tearDown(webView: BrowserWKWebView) {
            commandTask?.cancel()
            commandTask = nil
            webView.configuration.userContentController.removeScriptMessageHandler(
                forName: BrowserBridge.messageName,
                contentWorld: BrowserBridge.contentWorld
            )
            loadingObservation = nil
            progressObservation = nil
            canGoBackObservation = nil
            canGoForwardObservation = nil
            titleObservation = nil
            urlObservation = nil
        }

        private func handle(command: BrowserWebViewCommand) {
            guard let webView else { return }
            switch command {
            case let .navigate(url):
                webView.load(URLRequest(url: url))
            case .reload:
                webView.reload()
            case .stop:
                webView.stopLoading()
            case .back:
                webView.goBack()
            case .forward:
                webView.goForward()
            case let .setZoom(value):
                webView.pageZoom = value
            case let .scrollTo(y):
                evaluateInBridgeWorld(
                    "window.__muxyBrowserAPI && window.__muxyBrowserAPI.scrollTo(\(y));",
                    in: webView
                )
            case let .setInspectorMode(mode):
                evaluateInBridgeWorld(
                    "window.__muxyBrowserAPI && window.__muxyBrowserAPI.setMode('\(mode.rawValue)');",
                    in: webView
                )
            case let .applyStyleOverrides(overrides):
                pushStyleOverrides(overrides: overrides, into: webView)
            case let .find(query, forward):
                runFind(query: query, forward: forward, in: webView)
            case .clearFind:
                evaluateInBridgeWorld("window.getSelection && window.getSelection().removeAllRanges();", in: webView)
            }
        }

        private func runFind(query: String, forward: Bool, in webView: WKWebView) {
            let configuration = WKFindConfiguration()
            configuration.backwards = !forward
            configuration.caseSensitive = false
            configuration.wraps = true
            webView.find(query, configuration: configuration) { [weak self] result in
                Task { @MainActor in
                    guard let self else { return }
                    self.session.nav.findBar.lastResultFound = result.matchFound
                }
            }
        }

        private func pushStyleOverrides(overrides: [StyleOverride], into webView: WKWebView) {
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
            evaluateInBridgeWorld(
                "window.__muxyBrowserAPI && window.__muxyBrowserAPI.applyOverrides(\(json));",
                in: webView
            )
        }

        private func evaluateInBridgeWorld(_ script: String, in webView: WKWebView) {
            webView.evaluateJavaScript(script, in: nil, in: BrowserBridge.contentWorld) { result in
                if case let .failure(error) = result {
                    browserLogger.error("Bridge eval failed: \(error.localizedDescription, privacy: .public)")
                }
            }
        }

        func webView(
            _: WKWebView,
            decidePolicyFor navigationAction: WKNavigationAction
        ) async -> WKNavigationActionPolicy {
            guard let url = navigationAction.request.url else { return .cancel }
            guard BrowserURLNormalizer.isAllowedNavigationURL(url) else {
                Task { @MainActor in
                    session.nav.handleNavigationFailed(message: "Blocked navigation to \(url.scheme ?? "unknown") URL")
                }
                return .cancel
            }
            return .allow
        }

        func webView(_ webView: WKWebView, didFinish _: WKNavigation!) {
            Task { @MainActor in
                let url = webView.url?.absoluteString ?? session.nav.currentURL
                let title = webView.title ?? ""
                self.session.nav.handleNavigationFinished(
                    url: url,
                    title: title,
                    canGoBack: webView.canGoBack,
                    canGoForward: webView.canGoForward
                )
                if let pending = self.session.nav.pendingScrollRestore {
                    if BrowserURLNormalizer.canonical(pending.url) == BrowserURLNormalizer.canonical(url) {
                        self.evaluateInBridgeWorld("window.scrollTo(0, \(pending.y));", in: webView)
                    }
                    self.session.nav.pendingScrollRestore = nil
                }
                if self.session.inspector.inspectorMode != .off {
                    self.evaluateInBridgeWorld(
                        "window.__muxyBrowserAPI && window.__muxyBrowserAPI.setMode('\(self.session.inspector.inspectorMode.rawValue)');",
                        in: webView
                    )
                }
                let overrides = self.session.inspector.aggregatedStyleOverrides()
                if !overrides.isEmpty {
                    self.pushStyleOverrides(overrides: overrides, into: webView)
                }
            }
        }

        func webView(_: WKWebView, didFail _: WKNavigation!, withError error: Error) {
            Task { @MainActor in
                self.session.nav.handleNavigationFailed(message: error.localizedDescription)
            }
        }

        func webView(_: WKWebView, didFailProvisionalNavigation _: WKNavigation!, withError error: Error) {
            Task { @MainActor in
                self.session.nav.handleNavigationFailed(message: error.localizedDescription)
            }
        }

        func webView(
            _ webView: WKWebView,
            createWebViewWith _: WKWebViewConfiguration,
            for navigationAction: WKNavigationAction,
            windowFeatures _: WKWindowFeatures
        ) -> WKWebView? {
            guard let url = navigationAction.request.url,
                  BrowserURLNormalizer.isAllowedNavigationURL(url)
            else { return nil }
            webView.load(URLRequest(url: url))
            return nil
        }
    }
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
            forMainFrameOnly: true,
            in: BrowserBridge.contentWorld
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
            forMainFrameOnly: true,
            in: BrowserBridge.contentWorld
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
