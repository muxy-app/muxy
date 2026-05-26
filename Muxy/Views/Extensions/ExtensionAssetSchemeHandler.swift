import Foundation
import UniformTypeIdentifiers
import WebKit

final class ExtensionAssetSchemeHandler: NSObject, WKURLSchemeHandler {
    static let scheme = "muxy-ext"

    private let extensionID: String
    private let directory: URL

    init(extensionID: String, directory: URL) {
        self.extensionID = extensionID
        self.directory = directory.standardizedFileURL
    }

    func webView(_: WKWebView, start urlSchemeTask: WKURLSchemeTask) {
        guard let url = urlSchemeTask.request.url,
              url.scheme == Self.scheme
        else {
            urlSchemeTask.didFailWithError(URLError(.badURL))
            return
        }

        guard url.host == extensionID else {
            urlSchemeTask.didFailWithError(URLError(.badURL))
            return
        }

        let relativePath = url.path.isEmpty ? "" : String(url.path.dropFirst())
        let resolved = directory.appendingPathComponent(relativePath).standardizedFileURL

        guard resolved.path == directory.path || resolved.path.hasPrefix(directory.path + "/") else {
            urlSchemeTask.didFailWithError(URLError(.noPermissionsToReadFile))
            return
        }

        guard let data = try? Data(contentsOf: resolved) else {
            urlSchemeTask.didFailWithError(URLError(.fileDoesNotExist))
            return
        }

        let response = HTTPURLResponse(
            url: url,
            statusCode: 200,
            httpVersion: "HTTP/1.1",
            headerFields: [
                "Content-Type": Self.mimeType(for: resolved),
                "Content-Length": String(data.count),
                "Cache-Control": "no-store",
            ]
        )

        if let response {
            urlSchemeTask.didReceive(response)
        }
        urlSchemeTask.didReceive(data)
        urlSchemeTask.didFinish()
    }

    func webView(_: WKWebView, stop _: WKURLSchemeTask) {}

    private static func mimeType(for url: URL) -> String {
        let ext = url.pathExtension.lowercased()
        switch ext {
        case "html",
             "htm": return "text/html; charset=utf-8"
        case "js",
             "mjs": return "application/javascript; charset=utf-8"
        case "css": return "text/css; charset=utf-8"
        case "json": return "application/json; charset=utf-8"
        case "svg": return "image/svg+xml"
        case "png": return "image/png"
        case "jpg",
             "jpeg": return "image/jpeg"
        case "gif": return "image/gif"
        case "webp": return "image/webp"
        case "wasm": return "application/wasm"
        case "ico": return "image/x-icon"
        default:
            if let type = UTType(filenameExtension: ext),
               let mime = type.preferredMIMEType
            {
                return mime
            }
            return "application/octet-stream"
        }
    }
}
