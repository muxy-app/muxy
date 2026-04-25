import Foundation
import UniformTypeIdentifiers
import WebKit

final class MarkdownLocalImageSchemeHandler: NSObject, WKURLSchemeHandler {
    static let scheme = "muxy-md-image"

    private static let maxImageBytes: Int = 50 * 1024 * 1024

    private static let allowedMIMEPrefixes: [String] = [
        "image/",
    ]

    func webView(_: WKWebView, start urlSchemeTask: WKURLSchemeTask) {
        guard let url = urlSchemeTask.request.url,
              url.scheme == Self.scheme,
              let resolved = Self.resolveFileURL(from: url)
        else {
            urlSchemeTask.didFailWithError(URLError(.fileDoesNotExist))
            return
        }

        let mimeType = Self.mimeType(for: resolved)
        guard Self.isAllowedMIME(mimeType) else {
            urlSchemeTask.didFailWithError(URLError(.unsupportedURL))
            return
        }

        guard let fileSize = Self.fileSize(at: resolved), fileSize <= Self.maxImageBytes else {
            urlSchemeTask.didFailWithError(URLError(.dataLengthExceedsMaximum))
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
                "Content-Type": mimeType,
                "Content-Length": String(data.count),
                "Cache-Control": "no-cache",
            ]
        )

        if let response {
            urlSchemeTask.didReceive(response)
        }
        urlSchemeTask.didReceive(data)
        urlSchemeTask.didFinish()
    }

    func webView(_: WKWebView, stop _: WKURLSchemeTask) {}

    static func encodedURL(forBaseDirectory directory: String, relativePath: String) -> URL? {
        guard let encodedDir = directory.data(using: .utf8)?
            .base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
        else {
            return nil
        }
        let safeRelative = relativePath
            .split(separator: "/", omittingEmptySubsequences: true)
            .map { $0.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? String($0) }
            .joined(separator: "/")
        return URL(string: "\(scheme)://\(encodedDir)/\(safeRelative)")
    }

    private static func resolveFileURL(from url: URL) -> URL? {
        guard let host = url.host, !host.isEmpty else { return nil }
        let padded = host.replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
            .padding(toLength: ((host.count + 3) / 4) * 4, withPad: "=", startingAt: 0)
        guard let data = Data(base64Encoded: padded),
              let directory = String(data: data, encoding: .utf8)
        else {
            return nil
        }

        let baseRoot = URL(fileURLWithPath: directory, isDirectory: true)
            .standardizedFileURL
            .resolvingSymlinksInPath()
        let rawRelative = url.path.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        guard !rawRelative.isEmpty else { return nil }
        let decodedRelative = rawRelative.removingPercentEncoding ?? rawRelative

        let candidate = baseRoot.appendingPathComponent(decodedRelative)
            .standardizedFileURL
            .resolvingSymlinksInPath()
        let basePath = baseRoot.path
        let candidatePath = candidate.path
        guard candidatePath == basePath || candidatePath.hasPrefix(basePath + "/") else {
            return nil
        }
        return candidate
    }

    private static func fileSize(at url: URL) -> Int? {
        guard let values = try? url.resourceValues(forKeys: [.fileSizeKey]),
              let size = values.fileSize
        else {
            return nil
        }
        return size
    }

    private static func mimeType(for url: URL) -> String {
        if let utType = UTType(filenameExtension: url.pathExtension.lowercased()),
           let preferred = utType.preferredMIMEType
        {
            return preferred
        }
        return "application/octet-stream"
    }

    private static func isAllowedMIME(_ mimeType: String) -> Bool {
        let lowered = mimeType.lowercased()
        return allowedMIMEPrefixes.contains { lowered.hasPrefix($0) }
    }
}
