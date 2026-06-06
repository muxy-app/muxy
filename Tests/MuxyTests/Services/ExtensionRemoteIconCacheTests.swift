import AppKit
import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionRemoteIconCache", .serialized)
@MainActor
struct ExtensionRemoteIconCacheTests {
    @Test("decodes an SVG icon that AsyncImage cannot render")
    func decodesSVG() async {
        let svg = """
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" width="16" height="16">
        <rect width="16" height="16" fill="#3B82F6"/>
        </svg>
        """
        let cache = makeCache { _ in (200, "image/svg+xml", Data(svg.utf8)) }

        let image = await cache.image(for: URL(string: "https://muxy.test/icon")!)

        #expect(image != nil)
        #expect(image?.size.width == 16)
    }

    @Test("returns nil for a non-success response")
    func rejectsErrorStatus() async {
        let cache = makeCache { _ in (404, "text/plain", Data("nope".utf8)) }

        let image = await cache.image(for: URL(string: "https://muxy.test/missing")!)

        #expect(image == nil)
    }

    @Test("does not refetch a URL that previously failed")
    func cachesFailureNegatively() async {
        let counter = HitCounter()
        let cache = makeCache { _ in
            counter.increment()
            return (500, "text/plain", Data("boom".utf8))
        }
        let url = URL(string: "https://muxy.test/broken.png")!

        let first = await cache.image(for: url)
        let second = await cache.image(for: url)

        #expect(first == nil)
        #expect(second == nil)
        #expect(counter.value == 1)
    }

    @Test("fetches a URL only once and serves the cached image")
    func cachesAfterFirstFetch() async {
        let counter = HitCounter()
        let png = pngData()
        let cache = makeCache { _ in
            counter.increment()
            return (200, "image/png", png)
        }
        let url = URL(string: "https://muxy.test/icon.png")!

        let first = await cache.image(for: url)
        let second = await cache.image(for: url)

        #expect(first != nil)
        #expect(second != nil)
        #expect(counter.value == 1)
    }

    private func makeCache(
        handler: @escaping @Sendable (URLRequest) -> (Int, String, Data)
    ) -> ExtensionRemoteIconCache {
        IconStubURLProtocol.handler = handler
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [IconStubURLProtocol.self]
        return ExtensionRemoteIconCache(session: URLSession(configuration: configuration))
    }

    private func pngData() -> Data {
        let image = NSImage(size: NSSize(width: 4, height: 4))
        image.lockFocus()
        NSColor.red.setFill()
        NSRect(x: 0, y: 0, width: 4, height: 4).fill()
        image.unlockFocus()
        guard let tiff = image.tiffRepresentation,
              let rep = NSBitmapImageRep(data: tiff),
              let png = rep.representation(using: .png, properties: [:])
        else {
            return Data()
        }
        return png
    }
}

private final class HitCounter: @unchecked Sendable {
    private let lock = NSLock()
    private var count = 0

    func increment() {
        lock.lock()
        count += 1
        lock.unlock()
    }

    var value: Int {
        lock.lock()
        defer { lock.unlock() }
        return count
    }
}

private final class IconStubURLProtocol: URLProtocol {
    nonisolated(unsafe) static var handler: (@Sendable (URLRequest) -> (Int, String, Data))?

    override class func canInit(with _: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        guard let handler = Self.handler, let url = request.url else {
            client?.urlProtocol(self, didFailWithError: URLError(.badServerResponse))
            return
        }
        let (status, contentType, data) = handler(request)
        let response = HTTPURLResponse(
            url: url,
            statusCode: status,
            httpVersion: nil,
            headerFields: ["Content-Type": contentType]
        )!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: data)
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}
}
