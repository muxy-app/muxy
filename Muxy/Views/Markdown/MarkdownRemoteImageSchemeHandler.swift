import CryptoKit
import Foundation
import os
import UniformTypeIdentifiers
import WebKit

private let remoteImageLogger = Logger(subsystem: "app.muxy", category: "MarkdownRemoteImage")

private final class WKURLSchemeTaskBox: @unchecked Sendable {
    let schemeTask: WKURLSchemeTask
    init(schemeTask: WKURLSchemeTask) {
        self.schemeTask = schemeTask
    }
}

final class MarkdownRemoteImageSchemeHandler: NSObject, WKURLSchemeHandler {
    static let scheme = "muxy-md-remote"

    private static let maxImageBytes: Int = 50 * 1024 * 1024
    private static let cacheDirectoryName = "MarkdownImageCache"
    private static let allowedMIMEPrefixes: [String] = ["image/"]
    private static let userAgent = "Muxy/1.0 (Markdown Preview)"

    private static let urlSession: URLSession = {
        let config = URLSessionConfiguration.default
        config.timeoutIntervalForRequest = 20
        config.timeoutIntervalForResource = 60
        config.requestCachePolicy = .returnCacheDataElseLoad
        config.urlCache = URLCache(
            memoryCapacity: 8 * 1024 * 1024,
            diskCapacity: 64 * 1024 * 1024,
            diskPath: "muxy-markdown-remote-image-urlcache"
        )
        return URLSession(configuration: config)
    }()

    private let activeTasks = NSMapTable<URLSessionDataTask, AnyObject>.weakToStrongObjects()
    private let activeTasksLock = NSLock()

    func webView(_: WKWebView, start urlSchemeTask: WKURLSchemeTask) {
        guard let url = urlSchemeTask.request.url,
              url.scheme == Self.scheme,
              let remoteURL = Self.decodeRemoteURL(from: url)
        else {
            urlSchemeTask.didFailWithError(URLError(.badURL))
            return
        }

        if let cached = Self.readCache(for: remoteURL) {
            deliver(cached.data, mimeType: cached.mimeType, to: urlSchemeTask, originalURL: url)
            return
        }

        var request = URLRequest(url: remoteURL)
        request.setValue(Self.userAgent, forHTTPHeaderField: "User-Agent")
        let schemeTaskBox = WKURLSchemeTaskBox(schemeTask: urlSchemeTask)
        let task = Self.urlSession.dataTask(with: request) { [weak self] data, response, error in
            guard let self else { return }
            let outcome = FetchOutcome(
                data: data,
                response: response,
                error: error,
                schemeTaskBox: schemeTaskBox,
                remoteURL: remoteURL,
                originalURL: url
            )
            DispatchQueue.main.async {
                self.handleFetchResult(outcome)
            }
        }
        activeTasksLock.lock()
        activeTasks.setObject(urlSchemeTask as AnyObject, forKey: task)
        activeTasksLock.unlock()
        task.resume()
    }

    private struct FetchOutcome {
        let data: Data?
        let response: URLResponse?
        let error: Error?
        let schemeTaskBox: WKURLSchemeTaskBox
        let remoteURL: URL
        let originalURL: URL
    }

    @MainActor
    private func handleFetchResult(_ outcome: FetchOutcome) {
        let urlSchemeTask = outcome.schemeTaskBox.schemeTask
        let remoteURL = outcome.remoteURL
        let originalURL = outcome.originalURL
        let data = outcome.data
        let response = outcome.response
        let error = outcome.error
        activeTasksLock.lock()
        removeTaskMapping(for: urlSchemeTask)
        activeTasksLock.unlock()

        if let error {
            remoteImageLogger.debug(
                """
                Remote image fetch failed url=\(remoteURL.absoluteString, privacy: .public) \
                reason=\(error.localizedDescription, privacy: .public)
                """
            )
            failTask(urlSchemeTask, error: error)
            return
        }

        guard let data, !data.isEmpty else {
            failTask(urlSchemeTask, error: URLError(.zeroByteResource))
            return
        }
        guard data.count <= Self.maxImageBytes else {
            failTask(urlSchemeTask, error: URLError(.dataLengthExceedsMaximum))
            return
        }
        let mimeType = (response as? HTTPURLResponse)?.value(forHTTPHeaderField: "Content-Type")
            ?? response?.mimeType
            ?? Self.mimeType(forURL: remoteURL)
        let resolvedMIME = Self.resolvedMIMEType(mimeType, fallbackURL: remoteURL)
        guard Self.isAllowedMIME(resolvedMIME) else {
            failTask(urlSchemeTask, error: URLError(.unsupportedURL))
            return
        }

        Self.writeCache(data: data, mimeType: resolvedMIME, for: remoteURL)
        deliver(data, mimeType: resolvedMIME, to: urlSchemeTask, originalURL: originalURL)
    }

    func webView(_: WKWebView, stop urlSchemeTask: WKURLSchemeTask) {
        activeTasksLock.lock()
        let taskToCancel = findTask(for: urlSchemeTask)
        if let taskToCancel {
            activeTasks.removeObject(forKey: taskToCancel)
        }
        activeTasksLock.unlock()
        taskToCancel?.cancel()
    }

    private func findTask(for schemeTask: WKURLSchemeTask) -> URLSessionDataTask? {
        let enumerator = activeTasks.keyEnumerator()
        while let key = enumerator.nextObject() as? URLSessionDataTask {
            if (activeTasks.object(forKey: key) as AnyObject) === (schemeTask as AnyObject) {
                return key
            }
        }
        return nil
    }

    private func removeTaskMapping(for schemeTask: WKURLSchemeTask) {
        if let task = findTask(for: schemeTask) {
            activeTasks.removeObject(forKey: task)
        }
    }

    private func deliver(_ data: Data, mimeType: String, to task: WKURLSchemeTask, originalURL: URL) {
        let response = HTTPURLResponse(
            url: originalURL,
            statusCode: 200,
            httpVersion: "HTTP/1.1",
            headerFields: [
                "Content-Type": mimeType,
                "Content-Length": String(data.count),
                "Cache-Control": "max-age=31536000",
                "Access-Control-Allow-Origin": "*",
            ]
        )
        if let response {
            task.didReceive(response)
        }
        task.didReceive(data)
        task.didFinish()
    }

    private func failTask(_ task: WKURLSchemeTask, error: Error) {
        task.didFailWithError(error)
    }

    static func decodeRemoteURL(from url: URL) -> URL? {
        let token = url.lastPathComponent
        guard !token.isEmpty else { return nil }
        let padded = token.replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
            .padding(toLength: ((token.count + 3) / 4) * 4, withPad: "=", startingAt: 0)
        guard let data = Data(base64Encoded: padded),
              let decoded = String(data: data, encoding: .utf8),
              let resolved = URL(string: decoded),
              let scheme = resolved.scheme?.lowercased(),
              scheme == "http" || scheme == "https"
        else {
            return nil
        }
        return resolved
    }

    private static func cacheDirectory() -> URL? {
        guard let baseURL = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask).first else {
            return nil
        }
        let directory = baseURL.appendingPathComponent("Muxy", isDirectory: true).appendingPathComponent(
            cacheDirectoryName,
            isDirectory: true
        )
        if !FileManager.default.fileExists(atPath: directory.path) {
            try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        }
        return directory
    }

    private static func cacheKey(for url: URL) -> String {
        let digest = SHA256.hash(data: Data(url.absoluteString.utf8))
        return digest.map { String(format: "%02x", $0) }.joined()
    }

    private static func cacheURLs(for url: URL) -> (data: URL, meta: URL)? {
        guard let directory = cacheDirectory() else { return nil }
        let key = cacheKey(for: url)
        return (directory.appendingPathComponent(key + ".bin"), directory.appendingPathComponent(key + ".mime"))
    }

    static func readCache(for url: URL) -> (data: Data, mimeType: String)? {
        guard let urls = cacheURLs(for: url) else { return nil }
        guard let data = try? Data(contentsOf: urls.data) else { return nil }
        let mimeType = (try? String(contentsOf: urls.meta, encoding: .utf8))?
            .trimmingCharacters(in: .whitespacesAndNewlines) ?? mimeType(forURL: url)
        return (data, mimeType)
    }

    static func writeCache(data: Data, mimeType: String, for url: URL) {
        guard let urls = cacheURLs(for: url) else { return }
        try? data.write(to: urls.data, options: .atomic)
        try? mimeType.write(to: urls.meta, atomically: true, encoding: .utf8)
    }

    private static func resolvedMIMEType(_ mimeType: String, fallbackURL: URL) -> String {
        let trimmed = mimeType.split(separator: ";").first.map { $0.trimmingCharacters(in: .whitespaces) } ?? mimeType
        if !trimmed.isEmpty { return trimmed }
        return self.mimeType(forURL: fallbackURL)
    }

    private static func mimeType(forURL url: URL) -> String {
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
