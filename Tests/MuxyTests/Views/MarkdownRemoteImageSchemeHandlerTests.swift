import CryptoKit
import Foundation
import Testing
import WebKit

@testable import Muxy

@Suite("MarkdownRemoteImageSchemeHandler", .serialized)
struct MarkdownRemoteImageSchemeHandlerTests {
    @Test("decode rejects literal private and metadata hosts")
    func decodeRejectsLiteralPrivateHosts() throws {
        let urls = [
            try Self.schemeURL(for: "https://127.0.0.1/image.png"),
            try Self.schemeURL(for: "https://10.0.0.5/image.png"),
            try Self.schemeURL(for: "https://192.168.1.20/image.png"),
            try Self.schemeURL(for: "https://169.254.169.254/latest/meta-data"),
            try Self.schemeURL(for: "https://[fd00:ec2::254]/latest/meta-data"),
        ]

        for url in urls {
            #expect(MarkdownRemoteImageSchemeHandler.decodeRemoteURL(from: url) == nil)
        }
    }

    @Test("decode rejects non-HTTPS and empty host URLs")
    func decodeRejectsNonHTTPSAndEmptyHosts() throws {
        let urls = [
            try Self.schemeURL(for: "http://example.com/image.png"),
            try Self.schemeURL(for: "file:///tmp/image.png"),
            try Self.schemeURL(for: "https:///image.png"),
        ]

        for url in urls {
            #expect(MarkdownRemoteImageSchemeHandler.decodeRemoteURL(from: url) == nil)
        }
    }

    @Test("redirect rejects hosts that resolve to private addresses")
    func redirectRejectsPrivateResolvedHost() throws {
        let request = URLRequest(url: try #require(URL(string: "https://localhost/image.png")))

        #expect(MarkdownRemoteImageSchemeHandler.redirectRequestIfAllowed(request) == nil)
    }

    @Test("redirect allows literal public HTTPS address")
    func redirectAllowsLiteralPublicAddress() throws {
        let request = URLRequest(url: try #require(URL(string: "https://93.184.216.34/image.png")))

        #expect(MarkdownRemoteImageSchemeHandler.redirectRequestIfAllowed(request)?.url == request.url)
    }

    @Test("remote image MIME policy is explicit raster only")
    func remoteImageMIMEPolicyIsExplicitRasterOnly() {
        #expect(!MarkdownRemoteImageSchemeHandler.allowedMIMETypes.contains("image/"))
        #expect(!MarkdownRemoteImageSchemeHandler.allowedMIMETypes.contains("image/svg+xml"))
        #expect(MarkdownRemoteImageSchemeHandler.allowedMIMETypes.contains("image/png"))
        #expect(MarkdownRemoteImageSchemeHandler.allowedMIMETypes.contains("image/jpeg"))
    }

    @Test("cached disallowed MIME entries are rejected")
    func cachedDisallowedMIMEEntriesAreRejected() throws {
        let directory = try Self.temporaryCacheDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let url = try #require(URL(string: "https://example.com/\(UUID().uuidString).svg"))
        let urls = try Self.cacheURLs(for: url, in: directory)
        defer { Self.removeCacheFiles(urls) }

        MarkdownRemoteImageSchemeHandler.writeCache(
            data: Data(repeating: 1, count: 8),
            mimeType: "image/svg+xml",
            for: url,
            in: directory
        )

        #expect(MarkdownRemoteImageSchemeHandler.readCache(for: url, in: directory) == nil)
        #expect(!FileManager.default.fileExists(atPath: urls.data.path))
        #expect(!FileManager.default.fileExists(atPath: urls.meta.path))
    }

    @Test("host resolution is capped under burst load")
    @MainActor
    func hostResolutionIsCappedUnderBurstLoad() async throws {
        let probe = ResolverConcurrencyProbe()
        let handler = MarkdownRemoteImageSchemeHandler(
            hostResolver: { _ in
                probe.enter()
                probe.waitUntilReleased()
                probe.leave()
                return false
            },
            allowsRemoteImages: { true }
        )
        let tasks = try (0 ..< 12).map { index in
            FakeURLSchemeTask(url: try Self.schemeURL(for: "https://example\(index).com/image.png"))
        }

        for task in tasks {
            handler.start(task)
        }
        try await Task.sleep(nanoseconds: 200_000_000)

        #expect(probe.maxActive <= 4)

        for task in tasks {
            handler.stop(task)
        }
        probe.release()
        try await Task.sleep(nanoseconds: 200_000_000)
    }

    @Test("stop during host resolution prevents later task callbacks")
    @MainActor
    func stopDuringHostResolutionPreventsCallbacks() async throws {
        let resolverGate = ResolverGate()
        let handler = MarkdownRemoteImageSchemeHandler(
            hostResolver: { _ in
                resolverGate.markStarted()
                resolverGate.waitUntilReleased()
                resolverGate.markFinished()
                return false
            },
            allowsRemoteImages: { true }
        )
        let task = FakeURLSchemeTask(url: try Self.schemeURL(for: "https://example.com/image.png"))

        handler.start(task)
        await resolverGate.waitForStarted()

        handler.stop(task)
        resolverGate.release()
        await resolverGate.waitForFinished()
        await Self.drainMainQueue()

        #expect(task.callbackCount == 0)
    }

    @Test("expired cache entries are removed during reads")
    func expiredCacheEntriesAreRemovedDuringReads() throws {
        let directory = try Self.temporaryCacheDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let url = try #require(URL(string: "https://example.com/\(UUID().uuidString).png"))
        let urls = try Self.cacheURLs(for: url, in: directory)
        defer { Self.removeCacheFiles(urls) }

        MarkdownRemoteImageSchemeHandler.writeCache(
            data: Data(repeating: 1, count: 8),
            mimeType: "image/png",
            for: url,
            in: directory
        )
        let expired = Date(timeIntervalSinceNow: -(MarkdownRemoteImageSchemeHandler.cacheTTLSeconds + 60))
        try FileManager.default.setAttributes([.modificationDate: expired], ofItemAtPath: urls.data.path)

        #expect(MarkdownRemoteImageSchemeHandler.readCache(for: url, in: directory) == nil)
        #expect(!FileManager.default.fileExists(atPath: urls.data.path))
        #expect(!FileManager.default.fileExists(atPath: urls.meta.path))
    }

    @Test("cache pruning removes data and MIME files together")
    func cachePruningRemovesDataAndMIMEFilesTogether() throws {
        let directory = try Self.temporaryCacheDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let url = try #require(URL(string: "https://example.com/\(UUID().uuidString).png"))
        let urls = try Self.cacheURLs(for: url, in: directory)
        defer { Self.removeCacheFiles(urls) }

        MarkdownRemoteImageSchemeHandler.writeCache(
            data: Data(repeating: 1, count: 64),
            mimeType: "image/png",
            for: url,
            in: directory
        )
        let old = Date(timeIntervalSinceNow: -60)
        let recent = Date()
        try FileManager.default.setAttributes([.modificationDate: old], ofItemAtPath: urls.data.path)
        try FileManager.default.setAttributes([.modificationDate: recent], ofItemAtPath: urls.meta.path)

        MarkdownRemoteImageSchemeHandler.pruneCache(maxBytes: 16, in: directory)

        #expect(!FileManager.default.fileExists(atPath: urls.data.path))
        #expect(!FileManager.default.fileExists(atPath: urls.meta.path))
    }

    @Test("cache pruning excludes expired entries from size enforcement")
    func cachePruningExcludesExpiredEntriesFromSizeEnforcement() throws {
        let directory = try Self.temporaryCacheDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let expiredURL = try #require(URL(string: "https://example.com/\(UUID().uuidString).png"))
        let freshURL = try #require(URL(string: "https://example.com/\(UUID().uuidString).png"))
        let expiredURLs = try Self.cacheURLs(for: expiredURL, in: directory)
        let freshURLs = try Self.cacheURLs(for: freshURL, in: directory)
        defer {
            Self.removeCacheFiles(expiredURLs)
            Self.removeCacheFiles(freshURLs)
        }

        MarkdownRemoteImageSchemeHandler.writeCache(
            data: Data(repeating: 1, count: 64),
            mimeType: "image/png",
            for: expiredURL,
            in: directory
        )
        MarkdownRemoteImageSchemeHandler.writeCache(
            data: Data(repeating: 2, count: 64),
            mimeType: "image/png",
            for: freshURL,
            in: directory
        )
        let expired = Date(timeIntervalSinceNow: -(MarkdownRemoteImageSchemeHandler.cacheTTLSeconds + 3_600))
        let fresh = Date()
        for path in [expiredURLs.data.path, expiredURLs.meta.path] {
            try FileManager.default.setAttributes([.modificationDate: expired], ofItemAtPath: path)
        }
        for path in [freshURLs.data.path, freshURLs.meta.path] {
            try FileManager.default.setAttributes([.modificationDate: fresh], ofItemAtPath: path)
        }

        MarkdownRemoteImageSchemeHandler.pruneCache(maxBytes: 1, in: directory)

        #expect(!FileManager.default.fileExists(atPath: expiredURLs.data.path))
        #expect(!FileManager.default.fileExists(atPath: expiredURLs.meta.path))
        #expect(!FileManager.default.fileExists(atPath: freshURLs.data.path))
        #expect(!FileManager.default.fileExists(atPath: freshURLs.meta.path))
    }

    @Test("cache pruning removes unknown files in the managed directory")
    func cachePruningRemovesUnknownFilesInManagedDirectory() throws {
        let directory = try Self.temporaryCacheDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let unknown = directory.appendingPathComponent("\(UUID().uuidString).tmp")
        try Data(repeating: 3, count: 64).write(to: unknown)

        MarkdownRemoteImageSchemeHandler.pruneCache(maxBytes: 1, in: directory)

        #expect(!FileManager.default.fileExists(atPath: unknown.path))
    }

    private static func schemeURL(for remoteURL: String) throws -> URL {
        let token = Data(remoteURL.utf8).base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
        return try #require(URL(string: "\(MarkdownRemoteImageSchemeHandler.scheme)://image/\(token)"))
    }

    private static func temporaryCacheDirectory() throws -> URL {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(
            "muxy-markdown-cache-\(UUID().uuidString)",
            isDirectory: true
        )
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory
    }

    private static func cacheURLs(for url: URL, in directory: URL) throws -> (data: URL, meta: URL) {
        let digest = SHA256.hash(data: Data(url.absoluteString.utf8))
        let key = digest.map { String(format: "%02x", $0) }.joined()
        return (directory.appendingPathComponent(key + ".bin"), directory.appendingPathComponent(key + ".mime"))
    }

    @MainActor
    private static func drainMainQueue() async {
        await withCheckedContinuation { continuation in
            DispatchQueue.main.async {
                continuation.resume()
            }
        }
    }

    private static func removeCacheFiles(_ urls: (data: URL, meta: URL)) {
        try? FileManager.default.removeItem(at: urls.data)
        try? FileManager.default.removeItem(at: urls.meta)
    }
}

private final class ResolverGate: @unchecked Sendable {
    private let lock = NSLock()
    private let condition = NSCondition()
    private var started = false
    private var released = false
    private var finished = false
    private var continuations: [CheckedContinuation<Void, Never>] = []
    private var finishedContinuations: [CheckedContinuation<Void, Never>] = []

    func markStarted() {
        lock.lock()
        started = true
        let pending = continuations
        continuations.removeAll()
        lock.unlock()
        for continuation in pending {
            continuation.resume()
        }
    }

    func waitForStarted() async {
        await withCheckedContinuation { continuation in
            lock.lock()
            if started {
                lock.unlock()
                continuation.resume()
                return
            }
            continuations.append(continuation)
            lock.unlock()
        }
    }

    func markFinished() {
        lock.lock()
        finished = true
        let pending = finishedContinuations
        finishedContinuations.removeAll()
        lock.unlock()
        for continuation in pending {
            continuation.resume()
        }
    }

    func waitForFinished() async {
        await withCheckedContinuation { continuation in
            lock.lock()
            if finished {
                lock.unlock()
                continuation.resume()
                return
            }
            finishedContinuations.append(continuation)
            lock.unlock()
        }
    }

    func waitUntilReleased() {
        condition.lock()
        while !released {
            condition.wait()
        }
        condition.unlock()
    }

    func release() {
        condition.lock()
        released = true
        condition.broadcast()
        condition.unlock()
    }
}

private final class ResolverConcurrencyProbe: @unchecked Sendable {
    private let lock = NSLock()
    private let condition = NSCondition()
    private var active = 0
    private var released = false
    private(set) var maxActive = 0

    func enter() {
        lock.lock()
        active += 1
        maxActive = max(maxActive, active)
        lock.unlock()
    }

    func leave() {
        lock.lock()
        active -= 1
        lock.unlock()
    }

    func waitUntilReleased() {
        condition.lock()
        while !released {
            condition.wait()
        }
        condition.unlock()
    }

    func release() {
        condition.lock()
        released = true
        condition.broadcast()
        condition.unlock()
    }
}

private final class FakeURLSchemeTask: NSObject, WKURLSchemeTask {
    let request: URLRequest
    private let lock = NSLock()
    private var callbacks = 0

    var callbackCount: Int {
        lock.lock()
        defer { lock.unlock() }
        return callbacks
    }

    init(url: URL) {
        request = URLRequest(url: url)
    }

    func didReceive(_: URLResponse) {
        recordCallback()
    }

    func didReceive(_: Data) {
        recordCallback()
    }

    func didFinish() {
        recordCallback()
    }

    func didFailWithError(_: any Error) {
        recordCallback()
    }

    private func recordCallback() {
        lock.lock()
        callbacks += 1
        lock.unlock()
    }
}
