import CryptoKit
import Foundation
import os
import UniformTypeIdentifiers
import WebKit

private let remoteImageLogger = Logger(subsystem: "app.muxy", category: "MarkdownRemoteImage")

private final class WKURLSchemeTaskBox: @unchecked Sendable {
    let schemeTask: WKURLSchemeTask
    private let stateLock = NSLock()
    private var stoppedFlag = false

    init(schemeTask: WKURLSchemeTask) {
        self.schemeTask = schemeTask
    }

    var isStopped: Bool {
        stateLock.lock()
        defer { stateLock.unlock() }
        return stoppedFlag
    }

    func markStopped() {
        stateLock.lock()
        stoppedFlag = true
        stateLock.unlock()
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
        config.requestCachePolicy = .reloadIgnoringLocalCacheData
        config.urlCache = nil
        return URLSession(configuration: config)
    }()

    private let activeTasks = NSMapTable<URLSessionDataTask, WKURLSchemeTaskBox>.weakToStrongObjects()
    private let activeTasksLock = NSLock()

    func webView(_: WKWebView, start urlSchemeTask: WKURLSchemeTask) {
        guard MarkdownPreviewPreferences.allowRemoteImages else {
            urlSchemeTask.didFailWithError(URLError(.cancelled))
            return
        }

        guard let url = urlSchemeTask.request.url,
              url.scheme == Self.scheme,
              let remoteURL = Self.decodeRemoteURL(from: url)
        else {
            urlSchemeTask.didFailWithError(URLError(.badURL))
            return
        }

        let schemeTaskBox = WKURLSchemeTaskBox(schemeTask: urlSchemeTask)

        if let cached = Self.readCache(for: remoteURL) {
            deliver(cached.data, mimeType: cached.mimeType, to: schemeTaskBox, originalURL: url)
            return
        }

        var request = URLRequest(url: remoteURL)
        request.setValue(Self.userAgent, forHTTPHeaderField: "User-Agent")
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
        activeTasks.setObject(schemeTaskBox, forKey: task)
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
        let schemeTaskBox = outcome.schemeTaskBox
        let remoteURL = outcome.remoteURL
        let originalURL = outcome.originalURL
        let data = outcome.data
        let response = outcome.response
        let error = outcome.error
        activeTasksLock.lock()
        removeTaskMapping(for: schemeTaskBox)
        activeTasksLock.unlock()

        if let error {
            remoteImageLogger.debug(
                """
                Remote image fetch failed url=\(remoteURL.absoluteString, privacy: .public) \
                reason=\(error.localizedDescription, privacy: .public)
                """
            )
            failTask(schemeTaskBox, error: error)
            return
        }

        guard let data, !data.isEmpty else {
            failTask(schemeTaskBox, error: URLError(.zeroByteResource))
            return
        }
        guard data.count <= Self.maxImageBytes else {
            failTask(schemeTaskBox, error: URLError(.dataLengthExceedsMaximum))
            return
        }
        let mimeType = (response as? HTTPURLResponse)?.value(forHTTPHeaderField: "Content-Type")
            ?? response?.mimeType
            ?? Self.mimeType(forURL: remoteURL)
        let resolvedMIME = Self.resolvedMIMEType(mimeType, fallbackURL: remoteURL)
        guard Self.isAllowedMIME(resolvedMIME) else {
            failTask(schemeTaskBox, error: URLError(.unsupportedURL))
            return
        }

        Self.writeCache(data: data, mimeType: resolvedMIME, for: remoteURL)
        deliver(data, mimeType: resolvedMIME, to: schemeTaskBox, originalURL: originalURL)
    }

    func webView(_: WKWebView, stop urlSchemeTask: WKURLSchemeTask) {
        activeTasksLock.lock()
        let entry = findEntry(for: urlSchemeTask)
        if let entry {
            entry.box.markStopped()
            activeTasks.removeObject(forKey: entry.task)
        }
        activeTasksLock.unlock()
        entry?.task.cancel()
    }

    private func findEntry(for schemeTask: WKURLSchemeTask) -> (task: URLSessionDataTask, box: WKURLSchemeTaskBox)? {
        let enumerator = activeTasks.keyEnumerator()
        while let key = enumerator.nextObject() as? URLSessionDataTask {
            guard let box = activeTasks.object(forKey: key) else { continue }
            if box.schemeTask === schemeTask {
                return (key, box)
            }
        }
        return nil
    }

    private func removeTaskMapping(for box: WKURLSchemeTaskBox) {
        let enumerator = activeTasks.keyEnumerator()
        while let key = enumerator.nextObject() as? URLSessionDataTask {
            if activeTasks.object(forKey: key) === box {
                activeTasks.removeObject(forKey: key)
                return
            }
        }
    }

    private func deliver(_ data: Data, mimeType: String, to box: WKURLSchemeTaskBox, originalURL: URL) {
        guard !box.isStopped else { return }
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
            box.schemeTask.didReceive(response)
        }
        box.schemeTask.didReceive(data)
        box.schemeTask.didFinish()
    }

    private func failTask(_ box: WKURLSchemeTaskBox, error: Error) {
        guard !box.isStopped else { return }
        box.schemeTask.didFailWithError(error)
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
              scheme == "https",
              let host = resolved.host,
              !host.isEmpty
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
