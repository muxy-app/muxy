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
    nonisolated static let scheme = "muxy-md-remote"

    nonisolated static let maxImageBytes: Int = 50 * 1024 * 1024
    nonisolated static let cacheDirectoryName = "MarkdownImageCache"
    nonisolated static let cacheTTLSeconds: TimeInterval = 7 * 24 * 60 * 60
    nonisolated static let cacheSizeCapBytes: Int = 50 * 1024 * 1024
    nonisolated static let responseCacheMaxAgeSeconds: Int = 7 * 24 * 60 * 60
    nonisolated static let allowedMIMEPrefixes: [String] = ["image/"]
    nonisolated static let userAgent = "Muxy/1.0 (Markdown Preview)"

    nonisolated static let resolverQueue = DispatchQueue(
        label: "app.muxy.markdown-image-resolver",
        qos: .userInitiated,
        attributes: .concurrent
    )

    nonisolated static let cacheMaintenanceQueue = DispatchQueue(
        label: "app.muxy.markdown-image-cache",
        qos: .utility
    )

    nonisolated private static let sessionDelegate = SchemeHandlerSessionDelegate()

    nonisolated static let urlSession: URLSession = {
        let config = URLSessionConfiguration.default
        config.timeoutIntervalForRequest = 20
        config.timeoutIntervalForResource = 60
        config.requestCachePolicy = .reloadIgnoringLocalCacheData
        config.urlCache = nil
        return URLSession(configuration: config, delegate: sessionDelegate, delegateQueue: nil)
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

        Self.resolverQueue.async { [weak self] in
            guard let self else { return }
            let host = remoteURL.host ?? ""
            let allowed = PrivateNetworkGuard.hostResolvesToPublicAddress(host)
            DispatchQueue.main.async {
                guard !schemeTaskBox.isStopped else { return }
                if !allowed {
                    remoteImageLogger.debug(
                        "Rejected remote image: private/unresolved host=\(host, privacy: .public)"
                    )
                    self.failTask(schemeTaskBox, error: URLError(.badURL))
                    return
                }
                self.startFetch(remoteURL: remoteURL, originalURL: url, schemeTaskBox: schemeTaskBox)
            }
        }
    }

    private func startFetch(remoteURL: URL, originalURL: URL, schemeTaskBox: WKURLSchemeTaskBox) {
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
                originalURL: originalURL
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
                "Cache-Control": "max-age=\(Self.responseCacheMaxAgeSeconds)",
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

    nonisolated static func decodeRemoteURL(from url: URL) -> URL? {
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
              !host.isEmpty,
              !PrivateNetworkGuard.isLiteralPrivateAddress(host)
        else {
            return nil
        }
        return resolved
    }

    nonisolated static func redirectRequestIfAllowed(_ request: URLRequest) -> URLRequest? {
        guard let url = request.url,
              url.scheme?.lowercased() == "https",
              let host = url.host,
              !host.isEmpty,
              PrivateNetworkGuard.hostResolvesToPublicAddress(host)
        else {
            return nil
        }
        return request
    }

    nonisolated private static func cacheDirectory() -> URL? {
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

    nonisolated private static func cacheKey(for url: URL) -> String {
        let digest = SHA256.hash(data: Data(url.absoluteString.utf8))
        return digest.map { String(format: "%02x", $0) }.joined()
    }

    nonisolated private static func cacheURLs(for url: URL) -> (data: URL, meta: URL)? {
        guard let directory = cacheDirectory() else { return nil }
        let key = cacheKey(for: url)
        return (directory.appendingPathComponent(key + ".bin"), directory.appendingPathComponent(key + ".mime"))
    }

    nonisolated static func readCache(for url: URL) -> (data: Data, mimeType: String)? {
        guard let urls = cacheURLs(for: url) else { return nil }
        guard let attrs = try? FileManager.default.attributesOfItem(atPath: urls.data.path),
              let modified = attrs[.modificationDate] as? Date,
              Date().timeIntervalSince(modified) < cacheTTLSeconds,
              let data = try? Data(contentsOf: urls.data)
        else {
            return nil
        }
        let mimeType = (try? String(contentsOf: urls.meta, encoding: .utf8))?
            .trimmingCharacters(in: .whitespacesAndNewlines) ?? mimeType(forURL: url)
        return (data, mimeType)
    }

    nonisolated static func writeCache(data: Data, mimeType: String, for url: URL) {
        guard let urls = cacheURLs(for: url) else { return }
        try? data.write(to: urls.data, options: .atomic)
        try? mimeType.write(to: urls.meta, atomically: true, encoding: .utf8)
        cacheMaintenanceQueue.async { pruneCache(maxBytes: cacheSizeCapBytes) }
    }

    nonisolated static func pruneCache(maxBytes: Int) {
        guard let directory = cacheDirectory() else { return }
        let fm = FileManager.default
        guard let entries = try? fm.contentsOfDirectory(
            at: directory,
            includingPropertiesForKeys: [.fileSizeKey, .contentModificationDateKey],
            options: [.skipsHiddenFiles]
        )
        else { return }

        struct Entry {
            let url: URL
            let size: Int
            let modified: Date
        }

        var collected: [Entry] = []
        var total = 0
        for fileURL in entries {
            guard let values = try? fileURL.resourceValues(
                forKeys: [.fileSizeKey, .contentModificationDateKey]
            ),
                let size = values.fileSize,
                let modified = values.contentModificationDate
            else { continue }
            collected.append(Entry(url: fileURL, size: size, modified: modified))
            total += size
        }

        guard total > maxBytes else { return }
        let sorted = collected.sorted { $0.modified < $1.modified }
        for entry in sorted {
            if total <= maxBytes { break }
            try? fm.removeItem(at: entry.url)
            total -= entry.size
        }
    }

    nonisolated private static func resolvedMIMEType(_ mimeType: String, fallbackURL: URL) -> String {
        let trimmed = mimeType.split(separator: ";").first.map { $0.trimmingCharacters(in: .whitespaces) } ?? mimeType
        if !trimmed.isEmpty { return trimmed }
        return self.mimeType(forURL: fallbackURL)
    }

    nonisolated private static func mimeType(forURL url: URL) -> String {
        if let utType = UTType(filenameExtension: url.pathExtension.lowercased()),
           let preferred = utType.preferredMIMEType
        {
            return preferred
        }
        return "application/octet-stream"
    }

    nonisolated private static func isAllowedMIME(_ mimeType: String) -> Bool {
        let lowered = mimeType.lowercased()
        return allowedMIMEPrefixes.contains { lowered.hasPrefix($0) }
    }
}

private final class SchemeHandlerSessionDelegate: NSObject, URLSessionTaskDelegate {
    func urlSession(
        _: URLSession,
        task _: URLSessionTask,
        willPerformHTTPRedirection _: HTTPURLResponse,
        newRequest request: URLRequest,
        completionHandler: @escaping (URLRequest?) -> Void
    ) {
        completionHandler(MarkdownRemoteImageSchemeHandler.redirectRequestIfAllowed(request))
    }
}
