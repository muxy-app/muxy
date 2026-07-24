import Foundation
import WebKit

@MainActor
protocol BrowserElementInspecting: AnyObject {
    func inspectElement() -> Bool
    func closeInspector() -> Bool
}

@MainActor
final class BrowserWebViewRegistry {
    static let shared = BrowserWebViewRegistry()

    private final class WeakBox {
        weak var webView: WKWebView?
        init(_ webView: WKWebView) {
            self.webView = webView
        }
    }

    private var entries: [UUID: WeakBox] = [:]

    func register(_ webView: WKWebView, for tabID: UUID) {
        entries[tabID] = WeakBox(webView)
    }

    func unregister(_ tabID: UUID, ifMatches webView: WKWebView? = nil) {
        if let webView, entries[tabID]?.webView !== webView {
            return
        }
        entries[tabID] = nil
    }

    func unregister(_ webView: WKWebView) {
        for (tabID, entry) in entries where entry.webView === webView {
            entries[tabID] = nil
        }
    }

    func webView(for tabID: UUID) -> WKWebView? {
        guard let box = entries[tabID] else { return nil }
        guard let webView = box.webView else {
            entries[tabID] = nil
            return nil
        }
        return webView
    }

    @discardableResult
    func inspectElement(for tabID: UUID) -> Bool {
        guard let inspector = webView(for: tabID) as? any BrowserElementInspecting else { return false }
        return inspector.inspectElement()
    }
}
