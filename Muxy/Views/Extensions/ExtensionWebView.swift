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
    var surfaceStore: ExtensionTabSurfaceStore?

    @Environment(BrowserProfileStore.self) private var browserProfileStore: BrowserProfileStore?
    @Environment(\.overlayActive) private var overlayActive

    private static let mountBroker = ReparentingNSViewBroker<WKWebView> { _ in }

    func makeCoordinator() -> MountCoordinator {
        MountCoordinator()
    }

    func makeNSView(context: Context) -> ReparentingNSViewHost {
        let host = ReparentingNSViewHost()
        let surface = resolvedSurface(coordinator: context.coordinator)
        Self.mountBroker.register(
            claimID: context.coordinator.claimID,
            view: surface.webView,
            host: host,
            configuration: mountConfiguration(
                surface: surface,
                claimID: context.coordinator.claimID
            )
        )
        return host
    }

    func updateNSView(_ host: ReparentingNSViewHost, context: Context) {
        let surface = resolvedSurface(coordinator: context.coordinator)
        Self.mountBroker.update(
            claimID: context.coordinator.claimID,
            view: surface.webView,
            host: host,
            configuration: mountConfiguration(
                surface: surface,
                claimID: context.coordinator.claimID
            )
        )
    }

    static func dismantleNSView(_ host: ReparentingNSViewHost, coordinator: MountCoordinator) {
        guard mountBroker.release(claimID: coordinator.claimID, host: host) else { return }
        if let surface = coordinator.surface {
            surface.coordinator.deactivate(
                claimID: coordinator.claimID,
                in: surface.webView
            )
        }
        guard coordinator.surfaceStore == nil else {
            coordinator.surface = nil
            return
        }
        coordinator.surface?.retire()
        coordinator.surface = nil
    }

    private var identity: Surface.Identity {
        Surface.Identity(
            extensionID: extensionID,
            surfaceKey: LifecycleSurfaceKey(kind: surfaceKind, instanceID: instanceID),
            entryURL: entryURL
        )
    }

    private func resolvedSurface(coordinator: MountCoordinator) -> Surface {
        if let storedSurface = surfaceStore?.surface as? Surface,
           storedSurface.identity == identity
        {
            coordinator.surface = storedSurface
            coordinator.surfaceStore = surfaceStore
            return storedSurface
        }
        if let currentSurface = coordinator.surface,
           currentSurface.identity == identity
        {
            return currentSurface
        }
        surfaceStore?.surface?.retire()
        coordinator.surface?.retire()
        let surface = makeSurface()
        surfaceStore?.surface = surface
        coordinator.surface = surface
        coordinator.surfaceStore = surfaceStore
        return surface
    }

    private func makeSurface() -> Surface {
        let surfaceCoordinator = SurfaceCoordinator(surfaceKind: surfaceKind)
        guard let muxyExtension = ExtensionStore.shared.loadedExtension(id: extensionID) else {
            return Surface(
                identity: identity,
                webView: WKWebView(frame: .zero),
                coordinator: surfaceCoordinator
            )
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
        surfaceCoordinator.bridge = bridge
        let userContent = config.userContentController
        userContent.addScriptMessageHandler(
            bridge,
            contentWorld: .page,
            name: ExtensionWebBridge.messageHandlerName
        )
        let console = ExtensionConsoleHandler(extensionID: muxyExtension.id)
        userContent.add(console, name: ExtensionConsoleHandler.messageHandlerName)
        surfaceCoordinator.consoleHandler = console

        surfaceCoordinator.configureScriptInjection(
            extensionID: muxyExtension.id,
            tabInstanceID: instanceID,
            initialData: initialData
        )
        surfaceCoordinator.installBridgeScript(into: userContent)

        let webView = Self.makeWebView(configuration: config, surfaceKind: surfaceKind)
        webView.navigationDelegate = surfaceCoordinator
        webView.uiDelegate = surfaceCoordinator
        webView.load(URLRequest(url: entryURL))
        bridge.attach(to: webView)
        let surfaceKey = identity.surfaceKey
        bridge.bind(surfaceKey: surfaceKey)
        ExtensionSurfaceBridgeRegistry.shared.register(bridge, for: surfaceKey)
        surfaceCoordinator.surfaceKey = surfaceKey
        surfaceCoordinator.observeThemeChanges(for: webView)
        return Surface(
            identity: identity,
            webView: webView,
            coordinator: surfaceCoordinator,
            lifecycleBridge: bridge
        )
    }

    private func mountConfiguration(
        surface: Surface,
        claimID: UUID
    ) -> ReparentingNSViewBroker<WKWebView>.Configuration {
        .init(
            isEligible: { true },
            prepare: { webView in
                surface.coordinator.applyDataIfChanged(initialData, in: webView)
            },
            didMount: { webView, _, ownershipChanged in
                surface.coordinator.applyFocusIfChanged(
                    focused,
                    overlayActive: overlayActive,
                    in: webView,
                    claimID: claimID,
                    reset: ownershipChanged
                )
            }
        )
    }

    @MainActor
    final class MountCoordinator {
        let claimID = UUID()
        var surface: Surface?
        weak var surfaceStore: ExtensionTabSurfaceStore?
    }

    @MainActor
    final class Surface: ExtensionTabSurface {
        struct Identity: Equatable {
            let extensionID: String
            let surfaceKey: LifecycleSurfaceKey
            let entryURL: URL
        }

        let identity: Identity
        let webView: WKWebView
        let coordinator: SurfaceCoordinator
        private let lifecycleBridge: (any BeforeCloseAsking)?
        private var retired = false

        init(
            identity: Identity,
            webView: WKWebView,
            coordinator: SurfaceCoordinator,
            lifecycleBridge: (any BeforeCloseAsking)? = nil
        ) {
            self.identity = identity
            self.webView = webView
            self.coordinator = coordinator
            self.lifecycleBridge = lifecycleBridge
        }

        func retire() {
            guard !retired else { return }
            retired = true
            coordinator.stopObservingThemeChanges()
            coordinator.bridge?.dropAllEventSubscriptions()
            if let lifecycleBridge {
                ExtensionSurfaceBridgeRegistry.shared.unregister(
                    identity.surfaceKey,
                    ifMatches: lifecycleBridge
                )
            }
            webView.navigationDelegate = nil
            webView.uiDelegate = nil
            webView.configuration.userContentController.removeAllScriptMessageHandlers()
            webView.configuration.userContentController.removeAllUserScripts()
        }

        deinit {
            MainActor.assumeIsolated {
                retire()
            }
        }
    }

    @MainActor
    final class SurfaceCoordinator: NSObject, WKNavigationDelegate, WKUIDelegate {
        var bridge: ExtensionBridgeHandler?
        var consoleHandler: ExtensionConsoleHandler?
        var surfaceKey: LifecycleSurfaceKey?
        private let surfaceKind: LifecycleSurfaceKind
        private weak var webView: WKWebView?
        private var themeObserver: NSObjectProtocol?
        private var extensionID: String = ""
        private var tabInstanceID: String = ""
        private var initialData: ExtensionJSON?
        private var focused = false
        private var overlayActive = false
        private var activeClaimID: UUID?

        init(surfaceKind: LifecycleSurfaceKind) {
            self.surfaceKind = surfaceKind
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

        func applyFocusIfChanged(
            _ focused: Bool,
            overlayActive: Bool,
            in webView: WKWebView,
            claimID: UUID,
            reset: Bool = false
        ) {
            activeClaimID = claimID
            let focusChanged = focused != self.focused
            let overlayChanged = overlayActive != self.overlayActive
            guard reset || focusChanged || overlayChanged else { return }
            self.focused = focused
            self.overlayActive = overlayActive
            if focusChanged {
                pushFocusUpdate(in: webView)
            }
            updateFirstResponder(for: webView)
        }

        func deactivate(claimID: UUID, in webView: WKWebView) {
            guard activeClaimID == claimID else { return }
            activeClaimID = nil
            let focusChanged = focused
            focused = false
            overlayActive = false
            if focusChanged {
                pushFocusUpdate(in: webView)
            }
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
            ExtensionWebView.applyThemeBackground(to: webView, surfaceKind: surfaceKind)
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
    static func makeWebView(
        configuration: WKWebViewConfiguration,
        surfaceKind: LifecycleSurfaceKind
    ) -> WKWebView {
        let webView = WKWebView(frame: .zero, configuration: configuration)
        if surfaceKind == .popover {
            webView.setValue(false, forKey: "drawsBackground")
        } else {
            applyThemeBackground(to: webView, surfaceKind: surfaceKind)
        }
        return webView
    }

    static func applyThemeBackground(to webView: WKWebView, surfaceKind: LifecycleSurfaceKind) {
        guard surfaceKind != .popover else { return }
        webView.underPageBackgroundColor = MuxyTheme.nsBg
    }

    static func entryURL(for muxyExtension: MuxyExtension, entry: String) -> URL? {
        guard muxyExtension.resolveResource(entry) != nil else { return nil }
        let normalized = entry.hasPrefix("/") ? String(entry.dropFirst()) : entry
        return URL(string: "\(ExtensionAssetSchemeHandler.scheme)://\(muxyExtension.id)/\(normalized)")
    }
}
