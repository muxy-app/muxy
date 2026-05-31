import CryptoKit
import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionMarketplaceService", .serialized)
struct ExtensionMarketplaceServiceTests {
    @Test("decodes the data envelope for a single extension")
    func fetchDecodesEnvelope() async throws {
        let json = """
        {
            "data": {
                "name": "git-status",
                "description": "Show branch info.",
                "permissions": ["tabs:read", "tabs:write"],
                "author": { "name": "Saeed", "github": "saeedvaziry" },
                "homepage": "https://muxy.app",
                "repository": "https://github.com/muxy-app/git-status",
                "categories": ["git"],
                "icon_url": "https://muxy.app/extensions/git-status/icon",
                "screenshot_paths": [],
                "downloads": 10,
                "current_version": "1.4.2",
                "sha256": "abc",
                "size": 100,
                "download_url": "https://muxy.app/api/extensions/git-status/download"
            }
        }
        """
        let service = makeService { _ in (200, Data(json.utf8)) }

        let ext = try await service.fetch(name: "git-status")

        #expect(ext.name == "git-status")
        #expect(ext.currentVersion == "1.4.2")
        #expect(ext.resolvedPermissions == [.tabsRead, .tabsWrite])
    }

    @Test("maps 404 to notFound")
    func fetchMapsNotFound() async throws {
        let service = makeService { _ in (404, Data(#"{"message":"Not Found"}"#.utf8)) }

        await #expect(throws: MarketplaceError.notFound) {
            _ = try await service.fetch(name: "missing")
        }
    }

    @Test("download rejects a hash mismatch")
    func downloadRejectsHashMismatch() async throws {
        let payload = Data("zip-bytes".utf8)
        let service = makeService { _ in (200, payload) }
        let ext = makeExtension(sha256: "deadbeef", size: payload.count)

        await #expect(throws: MarketplaceError.hashMismatch) {
            _ = try await service.download(ext)
        }
    }

    @Test("download rejects a size mismatch")
    func downloadRejectsSizeMismatch() async throws {
        let payload = Data("zip-bytes".utf8)
        let service = makeService { _ in (200, payload) }
        let ext = makeExtension(sha256: sha256Hex(payload), size: payload.count + 1)

        await #expect(throws: MarketplaceError.sizeMismatch) {
            _ = try await service.download(ext)
        }
    }

    @Test("download returns verified bytes")
    func downloadReturnsVerifiedBytes() async throws {
        let payload = Data("zip-bytes".utf8)
        let service = makeService { _ in (200, payload) }
        let ext = makeExtension(sha256: sha256Hex(payload), size: payload.count)

        let data = try await service.download(ext)

        #expect(data == payload)
    }

    private func makeService(handler: @escaping @Sendable (URLRequest) -> (Int, Data)) -> ExtensionMarketplaceService {
        StubURLProtocol.handler = handler
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubURLProtocol.self]
        return ExtensionMarketplaceService(
            baseURL: URL(string: "https://muxy.test")!,
            session: URLSession(configuration: configuration)
        )
    }

    private func makeExtension(sha256: String, size: Int) -> MarketplaceExtension {
        let json = """
        {
            "name": "demo",
            "description": null,
            "permissions": [],
            "author": null,
            "homepage": null,
            "repository": null,
            "categories": [],
            "icon_url": null,
            "screenshot_paths": [],
            "downloads": 0,
            "current_version": "1.0.0",
            "sha256": "\(sha256)",
            "size": \(size),
            "download_url": "https://muxy.test/download"
        }
        """
        return try! JSONDecoder().decode(MarketplaceExtension.self, from: Data(json.utf8))
    }

    private func sha256Hex(_ data: Data) -> String {
        SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    }
}

private final class StubURLProtocol: URLProtocol {
    nonisolated(unsafe) static var handler: (@Sendable (URLRequest) -> (Int, Data))?

    override class func canInit(with _: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        guard let handler = Self.handler, let url = request.url else {
            client?.urlProtocol(self, didFailWithError: URLError(.badServerResponse))
            return
        }
        let (status, data) = handler(request)
        let response = HTTPURLResponse(url: url, statusCode: status, httpVersion: nil, headerFields: nil)!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: data)
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}
}
