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
        defer { stateLock.unlock() }
        stoppedFlag = true
    }
}

private final class RemoteImageTaskEntry: @unchecked Sendable {
    let box: WKURLSchemeTaskBox
    var dataTask: URLSessionDataTask?

    init(box: WKURLSchemeTaskBox) {
        self.box = box
    }
}

final class MarkdownRemoteImageSchemeHandler: NSObject, WKURLSchemeHandler {
    nonisolated static let scheme = "muxy-md-remote"

    nonisolated static let maxImageBytes: Int = 50 * 1024 * 1024
    nonisolated static let cacheDirectoryName = "MarkdownImageCache"
    nonisolated static let cacheTTLSeconds: TimeInterval = 7 * 24 * 60 * 60
    nonisolated static let cacheSizeCapBytes: Int = 50 * 1024 * 1024
    nonisolated static let responseCacheMaxAgeSeconds: Int = 7 * 24 * 60 * 60
    nonisolated static let allowedMIMETypes: [String] = [
        "image/png",
        "image/jpeg",
        "image/jpg",
        "image/gif",
        "image/webp",
        "image/avif",
        "image/heic",
        "image/heif",
        "image/bmp",
        "image/tiff",
        "image/x-icon",
        "image/vnd.microsoft.icon",
    ]
    nonisolated static let userAgent = "Muxy/1.0 (Markdown Preview)"
    nonisolated private static let maxConcurrentHostResolutions = 4
    nonisolated private static let resolverSemaphore = DispatchSemaphore(value: maxConcurrentHostResolutions)

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

    private var activeTasks: [ObjectIdentifier: RemoteImageTaskEntry] = [:]
    private let activeTasksLock = NSLock()
    private let hostResolver: @Sendable (String) -> Bool
    private let allowsRemoteImages: @Sendable () -> Bool

    init(
        hostResolver: @escaping @Sendable (String) -> Bool = PrivateNetworkGuard.hostResolvesToPublicAddress,
        allowsRemoteImages: @escaping @Sendable () -> Bool = { MarkdownPreviewPreferences.allowRemoteImages }
    ) {
        self.hostResolver = hostResolver
        self.allowsRemoteImages = allowsRemoteImages
        super.init()
    }

    func webView(_: WKWebView, start urlSchemeTask: WKURLSchemeTask) {
        start(urlSchemeTask)
    }

    func start(_ urlSchemeTask: WKURLSchemeTask) {
        guard allowsRemoteImages() else {
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

        registerTask(schemeTaskBox)
        Self.resolverQueue.async { [weak self] in
            guard let self else { return }
            guard !schemeTaskBox.isStopped else {
                DispatchQueue.main.async {
                    self.removeTaskMapping(for: schemeTaskBox)
                }
                return
            }
            let host = remoteURL.host ?? ""
            let allowed = Self.resolveWithBackpressure {
                guard !schemeTaskBox.isStopped else { return false }
                return self.hostResolver(host)
            }
            DispatchQueue.main.async {
                guard !schemeTaskBox.isStopped else {
                    self.removeTaskMapping(for: schemeTaskBox)
                    return
                }
                if !allowed {
                    remoteImageLogger.debug(
                        "Rejected remote image: private/unresolved host=\(host, privacy: .public)"
                    )
                    self.failTask(schemeTaskBox, error: URLError(.badURL))
                    self.removeTaskMapping(for: schemeTaskBox)
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
        guard let entry = activeTasks[ObjectIdentifier(schemeTaskBox.schemeTask)],
              entry.box === schemeTaskBox,
              !schemeTaskBox.isStopped
        else {
            activeTasksLock.unlock()
            task.cancel()
            return
        }
        entry.dataTask = task
        activeTasksLock.unlock()
        guard !schemeTaskBox.isStopped else {
            task.cancel()
            return
        }
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

    private struct CacheEntry {
        var dataURL: URL?
        var metaURL: URL?
        var size = 0
        var modified = Date.distantPast
        var dataModified: Date?

        var urls: (data: URL, meta: URL)? {
            guard let dataURL, let metaURL else { return nil }
            return (dataURL, metaURL)
        }
    }

    @MainActor
    private func handleFetchResult(_ outcome: FetchOutcome) {
        let schemeTaskBox = outcome.schemeTaskBox
        let remoteURL = outcome.remoteURL
        let originalURL = outcome.originalURL
        let data = outcome.data
        let response = outcome.response
        let error = outcome.error
        defer { removeTaskMapping(for: schemeTaskBox) }

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
        stop(urlSchemeTask)
    }

    func stop(_ urlSchemeTask: WKURLSchemeTask) {
        activeTasksLock.lock()
        let entry = activeTasks.removeValue(forKey: ObjectIdentifier(urlSchemeTask))
        if let entry {
            entry.box.markStopped()
        }
        activeTasksLock.unlock()
        entry?.dataTask?.cancel()
    }

    private func registerTask(_ box: WKURLSchemeTaskBox) {
        activeTasksLock.lock()
        activeTasks[ObjectIdentifier(box.schemeTask)] = RemoteImageTaskEntry(box: box)
        activeTasksLock.unlock()
    }

    private func removeTaskMapping(for box: WKURLSchemeTaskBox) {
        activeTasksLock.lock()
        let id = ObjectIdentifier(box.schemeTask)
        if activeTasks[id]?.box === box {
            activeTasks.removeValue(forKey: id)
        }
        activeTasksLock.unlock()
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

    nonisolated static func redirectRequestIfAllowed(
        _ request: URLRequest,
        completion: @escaping @Sendable (URLRequest?) -> Void
    ) {
        resolverQueue.async {
            let result = resolveWithBackpressure {
                redirectRequestIfAllowed(request)
            }
            completion(result)
        }
    }

    nonisolated private static func cacheDirectory() -> URL? {
        guard let baseURL = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask).first else {
            return nil
        }
        let directory = baseURL.appendingPathComponent("Muxy", isDirectory: true).appendingPathComponent(
            cacheDirectoryName,
            isDirectory: true
        )
        return prepareCacheDirectory(directory)
    }

    nonisolated private static func prepareCacheDirectory(_ directory: URL) -> URL? {
        if !FileManager.default.fileExists(atPath: directory.path) {
            do {
                try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            } catch {
                return nil
            }
        }
        return directory
    }

    nonisolated private static func cacheKey(for url: URL) -> String {
        let digest = SHA256.hash(data: Data(url.absoluteString.utf8))
        return digest.map { String(format: "%02x", $0) }.joined()
    }

    nonisolated private static func cacheURLs(for url: URL) -> (data: URL, meta: URL)? {
        guard let directory = cacheDirectory() else { return nil }
        return cacheURLs(for: url, in: directory)
    }

    nonisolated private static func cacheURLs(for url: URL, in directory: URL) -> (data: URL, meta: URL)? {
        guard let directory = prepareCacheDirectory(directory) else { return nil }
        let key = cacheKey(for: url)
        return (directory.appendingPathComponent(key + ".bin"), directory.appendingPathComponent(key + ".mime"))
    }

    nonisolated static func readCache(for url: URL) -> (data: Data, mimeType: String)? {
        guard let urls = cacheURLs(for: url) else { return nil }
        return readCache(for: url, urls: urls)
    }

    nonisolated static func readCache(for url: URL, in directory: URL) -> (data: Data, mimeType: String)? {
        guard let urls = cacheURLs(for: url, in: directory) else { return nil }
        return readCache(for: url, urls: urls)
    }

    nonisolated private static func readCache(
        for url: URL,
        urls: (data: URL, meta: URL)
    ) -> (data: Data, mimeType: String)? {
        guard let attrs = try? FileManager.default.attributesOfItem(atPath: urls.data.path),
              let modified = attrs[.modificationDate] as? Date
        else {
            removeCacheEntry(urls)
            return nil
        }
        guard Date().timeIntervalSince(modified) < cacheTTLSeconds,
              let data = try? Data(contentsOf: urls.data)
        else {
            removeCacheEntry(urls)
            return nil
        }
        let mimeType = (try? String(contentsOf: urls.meta, encoding: .utf8))?
            .trimmingCharacters(in: .whitespacesAndNewlines) ?? mimeType(forURL: url)
        guard isAllowedMIME(mimeType) else {
            removeCacheEntry(urls)
            return nil
        }
        return (data, mimeType)
    }

    nonisolated static func writeCache(data: Data, mimeType: String, for url: URL) {
        guard let urls = cacheURLs(for: url) else { return }
        writeCache(data: data, mimeType: mimeType, urls: urls, pruneAfterWrite: true)
    }

    nonisolated static func writeCache(data: Data, mimeType: String, for url: URL, in directory: URL) {
        guard let urls = cacheURLs(for: url, in: directory) else { return }
        writeCache(data: data, mimeType: mimeType, urls: urls, pruneAfterWrite: false)
    }

    nonisolated private static func writeCache(
        data: Data,
        mimeType: String,
        urls: (data: URL, meta: URL),
        pruneAfterWrite: Bool
    ) {
        try? data.write(to: urls.data, options: .atomic)
        try? mimeType.write(to: urls.meta, atomically: true, encoding: .utf8)
        if pruneAfterWrite {
            cacheMaintenanceQueue.async { pruneCache(maxBytes: cacheSizeCapBytes) }
        }
    }

    nonisolated static func pruneCache(maxBytes: Int) {
        guard let directory = cacheDirectory() else { return }
        pruneCache(maxBytes: maxBytes, in: directory)
    }

    nonisolated static func pruneCache(maxBytes: Int, in directory: URL) {
        guard let directory = prepareCacheDirectory(directory) else { return }
        let fm = FileManager.default
        guard let entries = try? fm.contentsOfDirectory(
            at: directory,
            includingPropertiesForKeys: [.fileSizeKey, .contentModificationDateKey],
            options: [.skipsHiddenFiles]
        )
        else { return }

        var collected: [String: CacheEntry] = [:]
        var total = 0
        var unknownEntries: [(url: URL, size: Int)] = []
        for fileURL in entries {
            guard let values = try? fileURL.resourceValues(
                forKeys: [.fileSizeKey, .contentModificationDateKey]
            ),
                let size = values.fileSize,
                let modified = values.contentModificationDate
            else { continue }
            let ext = fileURL.pathExtension
            if ext != "bin", ext != "mime" {
                unknownEntries.append((fileURL, size))
                total += size
                continue
            }
            let key = fileURL.deletingPathExtension().lastPathComponent
            var entry = collected[key] ?? CacheEntry()
            if ext == "bin" {
                entry.dataURL = fileURL
                entry.dataModified = modified
            } else {
                entry.metaURL = fileURL
            }
            entry.size += size
            entry.modified = max(entry.modified, modified)
            collected[key] = entry
            total += size
        }

        for entry in unknownEntries {
            try? fm.removeItem(at: entry.url)
            total -= entry.size
        }

        let now = Date()
        var remaining: [CacheEntry] = []
        for entry in collected.values {
            let expired = entry.dataModified.map { now.timeIntervalSince($0) >= cacheTTLSeconds } ?? true
            if expired || entry.urls == nil {
                removeCacheEntry(entry)
                total -= entry.size
            } else {
                remaining.append(entry)
            }
        }

        guard total > maxBytes else { return }
        let sorted = remaining.sorted { $0.modified < $1.modified }
        for entry in sorted {
            if total <= maxBytes { break }
            removeCacheEntry(entry)
            total -= entry.size
        }
    }

    nonisolated private static func removeCacheEntry(_ urls: (data: URL, meta: URL)) {
        try? FileManager.default.removeItem(at: urls.data)
        try? FileManager.default.removeItem(at: urls.meta)
    }

    nonisolated private static func removeCacheEntry(_ entry: CacheEntry) {
        if let dataURL = entry.dataURL {
            try? FileManager.default.removeItem(at: dataURL)
        }
        if let metaURL = entry.metaURL {
            try? FileManager.default.removeItem(at: metaURL)
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
        return allowedMIMETypes.contains(lowered)
    }

    nonisolated private static func resolveWithBackpressure<T>(_ work: () -> T) -> T {
        resolverSemaphore.wait()
        defer { resolverSemaphore.signal() }
        return work()
    }
}

private final class SchemeHandlerSessionDelegate: NSObject, URLSessionTaskDelegate {
    func urlSession(
        _: URLSession,
        task _: URLSessionTask,
        willPerformHTTPRedirection _: HTTPURLResponse,
        newRequest request: URLRequest,
        completionHandler: @escaping @Sendable (URLRequest?) -> Void
    ) {
        MarkdownRemoteImageSchemeHandler.redirectRequestIfAllowed(request) { allowedRequest in
            completionHandler(allowedRequest)
        }
    }
}
