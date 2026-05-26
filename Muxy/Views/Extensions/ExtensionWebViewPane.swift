import SwiftUI
import WebKit

struct ExtensionWebViewPane: View {
    let state: ExtensionTabState
    let focused: Bool
    let onFocus: () -> Void

    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore

    var body: some View {
        Group {
            if let muxyExtension = ExtensionStore.shared.loadedExtension(id: state.extensionID),
               let tabType = muxyExtension.manifest.tabType(id: state.tabTypeID),
               let entryURL = entryURL(for: muxyExtension, tabType: tabType)
            {
                ExtensionWebViewRepresentable(
                    state: state,
                    extension: muxyExtension,
                    entryURL: entryURL,
                    appState: appState,
                    projectStore: projectStore,
                    worktreeStore: worktreeStore,
                    onFocus: onFocus
                )
                .contentShape(Rectangle())
                .onTapGesture { onFocus() }
            } else {
                placeholder
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .contentShape(Rectangle())
                    .onTapGesture { onFocus() }
            }
        }
    }

    private var placeholder: some View {
        VStack(spacing: 8) {
            Image(systemName: "puzzlepiece.extension")
                .font(.system(size: 32, weight: .light))
            Text("Extension \(state.extensionID) is not loaded")
                .font(.headline)
            Text("Tab type: \(state.tabTypeID)")
                .font(.subheadline)
                .foregroundStyle(.secondary)
        }
        .padding()
    }

    private func entryURL(for muxyExtension: MuxyExtension, tabType: ExtensionTabType) -> URL? {
        guard muxyExtension.resolveResource(tabType.entry) != nil else { return nil }
        let normalizedEntry = tabType.entry.hasPrefix("/") ? String(tabType.entry.dropFirst()) : tabType.entry
        return URL(string: "\(ExtensionAssetSchemeHandler.scheme)://\(muxyExtension.id)/\(normalizedEntry)")
    }
}

private struct ExtensionWebViewRepresentable: NSViewRepresentable {
    let state: ExtensionTabState
    let `extension`: MuxyExtension
    let entryURL: URL
    let appState: AppState
    let projectStore: ProjectStore?
    let worktreeStore: WorktreeStore?
    let onFocus: () -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(onFocus: onFocus)
    }

    func makeNSView(context: Context) -> WKWebView {
        let config = WKWebViewConfiguration()
        config.setURLSchemeHandler(
            ExtensionAssetSchemeHandler(extensionID: `extension`.id, directory: `extension`.directory),
            forURLScheme: ExtensionAssetSchemeHandler.scheme
        )

        let bridge = ExtensionBridgeHandler(
            extensionID: `extension`.id,
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
        let console = ExtensionConsoleHandler(extensionID: `extension`.id)
        userContent.add(console, name: ExtensionConsoleHandler.messageHandlerName)
        context.coordinator.consoleHandler = console

        context.coordinator.configureScriptInjection(
            extensionID: `extension`.id,
            tabInstanceID: state.id.uuidString,
            initialData: state.initialData
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
