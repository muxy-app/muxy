import AppKit
import SwiftUI
import WebKit

final class HTMLPreviewWKWebView: WKWebView {
    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if event.modifierFlags.contains(.command),
           event.charactersIgnoringModifiers?.lowercased() == "r"
        {
            reload()
            return true
        }
        return super.performKeyEquivalent(with: event)
    }
}

struct HTMLPreviewWebView: NSViewRepresentable {
    let filePath: String

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeNSView(context: Context) -> HTMLPreviewWKWebView {
        let config = WKWebViewConfiguration()
        config.defaultWebpagePreferences.allowsContentJavaScript = true
        let webView = HTMLPreviewWKWebView(frame: .zero, configuration: config)
        load(into: webView, context: context)
        return webView
    }

    func updateNSView(_ webView: HTMLPreviewWKWebView, context: Context) {
        guard context.coordinator.loadedFilePath != filePath else { return }
        load(into: webView, context: context)
    }

    private func load(into webView: HTMLPreviewWKWebView, context: Context) {
        let fileURL = URL(fileURLWithPath: filePath)
        context.coordinator.loadedFilePath = filePath
        webView.loadFileURL(fileURL, allowingReadAccessTo: fileURL.deletingLastPathComponent())
    }

    final class Coordinator {
        var loadedFilePath: String?
    }
}
