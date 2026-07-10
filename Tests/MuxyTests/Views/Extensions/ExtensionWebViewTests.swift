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
}
