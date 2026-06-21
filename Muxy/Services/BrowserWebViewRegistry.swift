import Foundation
import WebKit

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

    func unregister(_ tabID: UUID) {
        entries[tabID] = nil
    }

    func webView(for tabID: UUID) -> WKWebView? {
        guard let box = entries[tabID] else { return nil }
        guard let webView = box.webView else {
            entries[tabID] = nil
            return nil
        }
        return webView
    }
}
