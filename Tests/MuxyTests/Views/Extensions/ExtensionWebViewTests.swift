import AppKit
import Testing
import WebKit

@testable import Muxy

@Suite("ExtensionWebView")
@MainActor
struct ExtensionWebViewTests {
    @Test(
        "non-popover surfaces use opaque themed rendering",
        arguments: [
            LifecycleSurfaceKind.tab,
            .panel,
            .sidebar,
            .modalWebview,
        ]
    )
    func nonPopoverSurfacesUseOpaqueThemedRendering(surfaceKind: LifecycleSurfaceKind) {
        let webView = ExtensionWebView.makeWebView(
            configuration: WKWebViewConfiguration(),
            surfaceKind: surfaceKind
        )

        #expect(webView.value(forKey: "drawsBackground") as? Bool == true)
        #expect(webView.underPageBackgroundColor?.isEqual(MuxyTheme.nsBg) == true)
    }

    @Test("popover surfaces preserve transparent native material")
    func popoverSurfacesPreserveTransparentNativeMaterial() {
        let webView = ExtensionWebView.makeWebView(
            configuration: WKWebViewConfiguration(),
            surfaceKind: .popover
        )

        #expect(webView.value(forKey: "drawsBackground") as? Bool == false)
    }

    @Test("theme updates refresh the native backing color")
    func themeUpdatesRefreshNativeBackingColor() {
        let webView = WKWebView(frame: .zero)
        webView.underPageBackgroundColor = .magenta

        ExtensionWebView.applyThemeBackground(to: webView, surfaceKind: .tab)

        #expect(webView.underPageBackgroundColor?.isEqual(MuxyTheme.nsBg) == true)
    }

    @Test("tab surface and lifecycle bridge survive inactive presentation")
    func tabSurfaceAndLifecycleBridgeSurviveInactivePresentation() async throws {
        var state: ExtensionTabState? = ExtensionTabState(
            extensionID: "test-extension",
            tabTypeID: "test-tab",
            projectPath: "/tmp/test",
            defaultTitle: "Test"
        )
        let key = LifecycleSurfaceKey(
            kind: .tab,
            instanceID: try #require(state?.id.uuidString)
        )
        let bridge = ExtensionSurfaceBridgeStub(verdict: .prevent)
        ExtensionSurfaceBridgeRegistry.shared.register(bridge, for: key)

        weak var retainedSurface: ExtensionWebView.Surface?
        do {
            let coordinator = ExtensionWebView.SurfaceCoordinator(surfaceKind: .tab)
            let surface = ExtensionWebView.Surface(
                identity: .init(
                    extensionID: "test-extension",
                    surfaceKey: key,
                    entryURL: try #require(URL(string: "muxy-extension://test-extension/index.html"))
                ),
                webView: WKWebView(frame: .zero),
                coordinator: coordinator,
                lifecycleBridge: bridge
            )
            retainedSurface = surface
            state?.surfaceStore.surface = surface
        }

        #expect(retainedSurface != nil)
        #expect(await ExtensionSurfaceBridgeRegistry.shared.requestBeforeClose(key) == .prevent)

        state = nil

        #expect(retainedSurface == nil)
        #expect(await ExtensionSurfaceBridgeRegistry.shared.requestBeforeClose(key) == .allow)
        #expect(bridge.failPendingCallCount == 1)
    }

    @Test("inactive tab surface publishes focus loss without overriding a replacement claim")
    func inactiveTabSurfacePublishesFocusLoss() {
        let coordinator = ExtensionWebView.SurfaceCoordinator(surfaceKind: .tab)
        let webView = JavaScriptRecordingWebView(frame: .zero)
        let firstClaimID = UUID()
        let replacementClaimID = UUID()

        coordinator.applyFocusIfChanged(
            true,
            overlayActive: false,
            in: webView,
            claimID: firstClaimID
        )
        coordinator.applyFocusIfChanged(
            true,
            overlayActive: false,
            in: webView,
            claimID: replacementClaimID,
            reset: true
        )
        coordinator.deactivate(claimID: firstClaimID, in: webView)

        #expect(webView.scripts.count == 1)

        coordinator.deactivate(claimID: replacementClaimID, in: webView)

        #expect(webView.scripts.count == 2)
        #expect(webView.scripts.last?.contains("__muxyApplyFocus(false)") == true)
    }
}

@MainActor
private final class JavaScriptRecordingWebView: WKWebView {
    private(set) var scripts: [String] = []

    override func evaluateJavaScript(
        _ javaScriptString: String,
        completionHandler: (@MainActor @Sendable (Any?, (any Error)?) -> Void)? = nil
    ) {
        scripts.append(javaScriptString)
        completionHandler?(nil, nil)
    }
}

@MainActor
private final class ExtensionSurfaceBridgeStub: BeforeCloseAsking {
    let verdict: LifecycleVerdict
    private(set) var failPendingCallCount = 0

    init(verdict: LifecycleVerdict) {
        self.verdict = verdict
    }

    func requestBeforeClose(reason _: LifecycleSurfaceKind, instanceID _: String) async -> LifecycleVerdict {
        verdict
    }

    func failPendingLifecycle() {
        failPendingCallCount += 1
    }
}
