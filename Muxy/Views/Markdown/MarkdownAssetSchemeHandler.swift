import Foundation
import WebKit

final class MarkdownAssetSchemeHandler: NSObject, WKURLSchemeHandler {
    static let scheme = "muxy-asset"
    static let host = "markdown"

    private static let allowedFiles: Set<String> = [
        "marked.min.js",
        "mermaid.min.js",
        "markdown-renderer.js",
    ]

    private static let mimeTypes: [String: String] = [
        "js": "application/javascript",
    ]

    func webView(_: WKWebView, start urlSchemeTask: WKURLSchemeTask) {
        guard let url = urlSchemeTask.request.url,
              url.scheme == Self.scheme,
              url.host == Self.host
        else {
            urlSchemeTask.didFailWithError(URLError(.badURL))
            return
        }

        let filename = url.lastPathComponent
        guard Self.allowedFiles.contains(filename),
              let resourceURL = Bundle.module.url(forResource: filename, withExtension: nil),
              let data = try? Data(contentsOf: resourceURL)
        else {
            urlSchemeTask.didFailWithError(URLError(.fileDoesNotExist))
            return
        }

        let ext = (filename as NSString).pathExtension.lowercased()
        let mimeType = Self.mimeTypes[ext] ?? "application/octet-stream"

        let response = HTTPURLResponse(
            url: url,
            statusCode: 200,
            httpVersion: "HTTP/1.1",
            headerFields: [
                "Content-Type": mimeType,
                "Content-Length": String(data.count),
                "Cache-Control": "max-age=31536000",
            ]
        )

        if let response {
            urlSchemeTask.didReceive(response)
        }
        urlSchemeTask.didReceive(data)
        urlSchemeTask.didFinish()
    }

    func webView(_: WKWebView, stop _: WKURLSchemeTask) {}
}
