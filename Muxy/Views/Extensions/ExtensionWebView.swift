import SwiftUI
import WebKit

struct ExtensionWebView: NSViewRepresentable {
    let extensionID: String
    let instanceID: String
    let entryURL: URL
    let initialData: ExtensionJSON?
    let appState: AppState
    let projectStore: ProjectStore?
    let worktreeStore: WorktreeStore?
    let onFocus: () -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(onFocus: onFocus)
    }

    func makeNSView(context: Context) -> WKWebView {
        guard let muxyExtension = ExtensionStore.shared.loadedExtension(id: extensionID) else {
            return WKWebView(frame: .zero)
        }

        let config = WKWebViewConfiguration()
        config.setURLSchemeHandler(
            ExtensionAssetSchemeHandler(extensionID: muxyExtension.id, directory: muxyExtension.directory),
            forURLScheme: ExtensionAssetSchemeHandler.scheme
        )

        let bridge = ExtensionBridgeHandler(
            extensionID: muxyExtension.id,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore
        )
        context.coordinator.bridge = bridge

        let userContent = config.userContentController
        userContent.addScriptMessageHandler(
            bridge,
            contentWorld: .page,
            name: ExtensionWebBridge.messageHandlerName
        )
        let console = ExtensionConsoleHandler(extensionID: muxyExtension.id)
        userContent.add(console, name: ExtensionConsoleHandler.messageHandlerName)
        context.coordinator.consoleHandler = console

        context.coordinator.configureScriptInjection(
            extensionID: muxyExtension.id,
            tabInstanceID: instanceID,
            initialData: initialData
        )
        context.coordinator.installBridgeScript(into: userContent)

        let webView = WKWebView(frame: .zero, configuration: config)
        webView.navigationDelegate = context.coordinator
        webView.uiDelegate = context.coordinator
        webView.setValue(false, forKey: "drawsBackground")
        webView.load(URLRequest(url: entryURL))
        bridge.attach(to: webView)
        context.coordinator.observeThemeChanges(for: webView)
        return webView
    }

    func updateNSView(_: WKWebView, context _: Context) {}

    static func dismantleNSView(_ webView: WKWebView, coordinator: Coordinator) {
        coordinator.stopObservingThemeChanges()
        coordinator.bridge?.dropAllEventSubscriptions()
        webView.navigationDelegate = nil
        webView.uiDelegate = nil
        webView.configuration.userContentController.removeAllScriptMessageHandlers()
        webView.configuration.userContentController.removeAllUserScripts()
    }

    @MainActor
    final class Coordinator: NSObject, WKNavigationDelegate, WKUIDelegate {
        var bridge: ExtensionBridgeHandler?
        var consoleHandler: ExtensionConsoleHandler?
        let onFocus: () -> Void
        private weak var webView: WKWebView?
        private weak var userContent: WKUserContentController?
        private var themeObserver: NSObjectProtocol?
        private var extensionID: String = ""
        private var tabInstanceID: String = ""
        private var initialData: ExtensionJSON?

        init(onFocus: @escaping () -> Void) {
            self.onFocus = onFocus
        }

        func configureScriptInjection(
            extensionID: String,
            tabInstanceID: String,
            initialData: ExtensionJSON?
        ) {
            self.extensionID = extensionID
            self.tabInstanceID = tabInstanceID
            self.initialData = initialData
        }

        func installBridgeScript(into userContent: WKUserContentController) {
            self.userContent = userContent
            reinstallBridgeScript()
        }

        private func reinstallBridgeScript() {
            guard let userContent else { return }
            userContent.removeAllUserScripts()
            userContent.addUserScript(WKUserScript(
                source: ExtensionWebBridge.script(
                    extensionID: extensionID,
                    tabInstanceID: tabInstanceID,
                    data: initialData,
                    theme: ExtensionThemeSnapshot.current()
                ),
                injectionTime: .atDocumentStart,
                forMainFrameOnly: true
            ))
        }

        func observeThemeChanges(for webView: WKWebView) {
            self.webView = webView
            themeObserver = NotificationCenter.default.addObserver(
                forName: .themeDidChange,
                object: nil,
                queue: .main
            ) { [weak self] _ in
                MainActor.assumeIsolated {
                    self?.pushThemeUpdate()
                }
            }
        }

        func stopObservingThemeChanges() {
            if let observer = themeObserver {
                NotificationCenter.default.removeObserver(observer)
                themeObserver = nil
            }
        }

        private func pushThemeUpdate() {
            guard let webView else { return }
            let theme = ExtensionThemeSnapshot.current()
            let script = ExtensionWebBridge.themeUpdateScript(theme: theme)
            webView.evaluateJavaScript(script, completionHandler: nil)
        }

        func webView(
            _: WKWebView,
            decidePolicyFor navigationAction: WKNavigationAction,
            decisionHandler: @escaping (WKNavigationActionPolicy) -> Void
        ) {
            guard let url = navigationAction.request.url else {
                decisionHandler(.allow)
                return
            }
            if url.scheme == ExtensionAssetSchemeHandler.scheme {
                decisionHandler(.allow)
                return
            }
            if url.scheme == "about" {
                decisionHandler(.allow)
                return
            }
            decisionHandler(.cancel)
        }

        func webView(
            _: WKWebView,
            createWebViewWith _: WKWebViewConfiguration,
            for _: WKNavigationAction,
            windowFeatures _: WKWindowFeatures
        ) -> WKWebView? {
            nil
        }

        func webView(_: WKWebView, didCommit _: WKNavigation!) {
            bridge?.dropAllEventSubscriptions()
        }
    }
}

extension ExtensionWebView {
    static func entryURL(for muxyExtension: MuxyExtension, entry: String) -> URL? {
        guard muxyExtension.resolveResource(entry) != nil else { return nil }
        let normalized = entry.hasPrefix("/") ? String(entry.dropFirst()) : entry
        return URL(string: "\(ExtensionAssetSchemeHandler.scheme)://\(muxyExtension.id)/\(normalized)")
    }
}
