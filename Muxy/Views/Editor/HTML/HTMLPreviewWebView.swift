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
    let backgroundColor: NSColor

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
        let backgroundHex = Self.colorToHex(backgroundColor)
        guard context.coordinator.loadedFilePath != filePath || context.coordinator.loadedBackgroundHex != backgroundHex else {
            return
        }
        load(into: webView, context: context)
    }

    private func load(into webView: HTMLPreviewWKWebView, context: Context) {
        let fileURL = URL(fileURLWithPath: filePath)
        context.coordinator.loadedFilePath = filePath
        context.coordinator.loadedBackgroundHex = Self.colorToHex(backgroundColor)
        if fileURL.pathExtension.lowercased() == "svg" {
            webView.loadHTMLString(
                Self.svgPreviewHTML(fileURL: fileURL, backgroundColor: backgroundColor),
                baseURL: fileURL.deletingLastPathComponent()
            )
            return
        }
        webView.loadFileURL(fileURL, allowingReadAccessTo: fileURL.deletingLastPathComponent())
    }

    static func svgPreviewHTML(fileURL: URL, backgroundColor: NSColor) -> String {
        let src = svgDataSource(fileURL: fileURL)
        let backgroundHex = colorToHex(backgroundColor)
        return """
        <!doctype html>
        <html>
        <head>
        <meta name="viewport" content="width=device-width, initial-scale=1">
        <style>
        html, body {
            width: 100%;
            height: 100%;
            margin: 0;
            overflow: hidden;
            background: #\(backgroundHex);
        }
        body {
            display: flex;
            align-items: center;
            justify-content: center;
            box-sizing: border-box;
            padding: 24px;
        }
        img {
            display: block;
            width: 100%;
            height: 100%;
            object-fit: contain;
        }
        </style>
        </head>
        <body>
        <img src="\(src)" alt="SVG preview">
        </body>
        </html>
        """
    }

    private static func svgDataSource(fileURL: URL) -> String {
        guard let data = try? Data(contentsOf: fileURL) else { return "" }
        return "data:image/svg+xml;base64,\(data.base64EncodedString())"
    }

    private static func colorToHex(_ color: NSColor) -> String {
        let spaces: [NSColorSpace] = [.sRGB, .extendedSRGB, .deviceRGB, .genericRGB]
        for space in spaces {
            if let rgb = color.usingColorSpace(space) {
                let red = Int(round(rgb.redComponent * 255))
                let green = Int(round(rgb.greenComponent * 255))
                let blue = Int(round(rgb.blueComponent * 255))
                return String(
                    format: "%02X%02X%02X",
                    max(0, min(255, red)),
                    max(0, min(255, green)),
                    max(0, min(255, blue))
                )
            }
        }
        return "1E1E1E"
    }

    final class Coordinator {
        var loadedFilePath: String?
        var loadedBackgroundHex: String?
    }
}
