import SwiftUI
import WebKit

struct ExtensionWebView: NSViewRepresentable {
    let extensionID: String
    let instanceID: String
    let surfaceKind: LifecycleSurfaceKind
    let entryURL: URL
    let initialData: ExtensionJSON?
    let appState: AppState
    let projectStore: ProjectStore?
    let worktreeStore: WorktreeStore?
    let projectGroupStore: ProjectGroupStore?
    let focused: Bool
    let onFocus: () -> Void

    @Environment(BrowserProfileStore.self) private var browserProfileStore: BrowserProfileStore?
    @Environment(\.overlayActive) private var overlayActive

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
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore,
            browserProfileStore: browserProfileStore
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
        let surfaceKey = LifecycleSurfaceKey(kind: surfaceKind, instanceID: instanceID)
        bridge.bind(surfaceKey: surfaceKey)
        ExtensionSurfaceBridgeRegistry.shared.register(bridge, for: surfaceKey)
        context.coordinator.surfaceKey = surfaceKey
        context.coordinator.observeThemeChanges(for: webView)
        return webView
    }

    func updateNSView(_ webView: WKWebView, context: Context) {
        context.coordinator.applyDataIfChanged(initialData, in: webView)
        context.coordinator.applyFocusIfChanged(focused, overlayActive: overlayActive, in: webView)
    }

    static func dismantleNSView(_ webView: WKWebView, coordinator: Coordinator) {
        coordinator.stopObservingThemeChanges()
        coordinator.bridge?.dropAllEventSubscriptions()
        if let surfaceKey = coordinator.surfaceKey {
            ExtensionSurfaceBridgeRegistry.shared.unregister(surfaceKey)
        }
        webView.navigationDelegate = nil
        webView.uiDelegate = nil
        webView.configuration.userContentController.removeAllScriptMessageHandlers()
        webView.configuration.userContentController.removeAllUserScripts()
    }

    @MainActor
    final class Coordinator: NSObject, WKNavigationDelegate, WKUIDelegate {
        var bridge: ExtensionBridgeHandler?
        var consoleHandler: ExtensionConsoleHandler?
        var surfaceKey: LifecycleSurfaceKey?
        let onFocus: () -> Void
        private weak var webView: WKWebView?
        private var themeObserver: NSObjectProtocol?
        private var extensionID: String = ""
        private var tabInstanceID: String = ""
        private var initialData: ExtensionJSON?
        private var focused = false
        private var overlayActive = false

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

        func applyDataIfChanged(_ data: ExtensionJSON?, in webView: WKWebView) {
            guard data != initialData else { return }
            initialData = data
            let script = ExtensionWebBridge.dataUpdateScript(data: data)
            webView.evaluateJavaScript(script, completionHandler: nil)
        }

        func applyFocusIfChanged(_ focused: Bool, overlayActive: Bool, in webView: WKWebView) {
            let focusChanged = focused != self.focused
            let overlayChanged = overlayActive != self.overlayActive
            guard focusChanged || overlayChanged else { return }
            self.focused = focused
            self.overlayActive = overlayActive
            if focusChanged { pushFocusUpdate(in: webView) }
            updateFirstResponder(for: webView)
        }

        private func pushFocusUpdate(in webView: WKWebView) {
            let script = ExtensionWebBridge.focusUpdateScript(focused: focused)
            webView.evaluateJavaScript(script, completionHandler: nil)
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
            decisionHandler: @escaping @MainActor @Sendable (WKNavigationActionPolicy) -> Void
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
            bridge?.failPendingLifecycle()
            pushThemeUpdate()
        }

        func webView(_ webView: WKWebView, didFinish _: WKNavigation!) {
            guard focused else { return }
            pushFocusUpdate(in: webView)
            updateFirstResponder(for: webView)
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
