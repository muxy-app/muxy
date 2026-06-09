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

    @Test("list decodes simplePaginate payload and derives hasNextPage from next link")
    func listDecodesCatalogPage() async throws {
        let json = """
        {
            "data": [
                {
                    "name": "git-status",
                    "description": "Show branch info.",
                    "author": { "name": "Saeed", "github": "saeedvaziry" },
                    "categories": ["git"],
                    "official": true,
                    "downloads": 4213,
                    "version": "1.4.2",
                    "icon_url": "https://muxy.app/extensions/git-status/icon"
                }
            ],
            "links": { "first": "…?page=1", "last": null, "prev": null, "next": "…?page=2" },
            "meta": { "current_page": 1, "from": 1, "per_page": 24, "to": 24 }
        }
        """
        let service = makeService { _ in (200, Data(json.utf8)) }

        let page = try await service.list(query: ExtensionCatalogQuery())

        #expect(page.items.count == 1)
        #expect(page.items.first?.name == "git-status")
        #expect(page.items.first?.official == true)
        #expect(page.items.first?.version == "1.4.2")
        #expect(page.hasNextPage)
    }

    @Test("list reports no next page when the next link is null")
    func listDetectsLastPage() async throws {
        let json = """
        {
            "data": [],
            "links": { "first": "…?page=1", "last": null, "prev": "…?page=1", "next": null },
            "meta": { "current_page": 2, "from": null, "per_page": 24, "to": null }
        }
        """
        let service = makeService { _ in (200, Data(json.utf8)) }

        let page = try await service.list(query: ExtensionCatalogQuery(page: 2))

        #expect(page.items.isEmpty)
        #expect(!page.hasNextPage)
    }

    @Test("list encodes search, sort, category, and pagination as query parameters")
    func listEncodesQueryParameters() async throws {
        let captured = RequestBox()
        let service = makeService { request in
            captured.url = request.url
            return (200, Data(#"{"data":[],"links":{"next":null}}"#.utf8))
        }

        _ = try await service.list(query: ExtensionCatalogQuery(
            search: "git",
            sort: .popular,
            category: "productivity",
            official: true,
            page: 3,
            perPage: 24
        ))

        let items = queryItems(captured.url)
        #expect(items["search"] == "git")
        #expect(items["sort"] == "popular")
        #expect(items["category"] == "productivity")
        #expect(items["official"] == "true")
        #expect(items["page"] == "3")
        #expect(items["per_page"] == "24")
    }

    @Test("list clamps per_page to the supported range")
    func listClampsPageSize() async throws {
        let captured = RequestBox()
        let service = makeService { request in
            captured.url = request.url
            return (200, Data(#"{"data":[],"links":{"next":null}}"#.utf8))
        }

        _ = try await service.list(query: ExtensionCatalogQuery(perPage: 500))

        #expect(queryItems(captured.url)["per_page"] == "50")
    }

    @Test("list omits blank search and absent filters")
    func listOmitsEmptyParameters() async throws {
        let captured = RequestBox()
        let service = makeService { request in
            captured.url = request.url
            return (200, Data(#"{"data":[],"links":{"next":null}}"#.utf8))
        }

        _ = try await service.list(query: ExtensionCatalogQuery(search: "   "))

        let items = queryItems(captured.url)
        #expect(items["search"] == nil)
        #expect(items["category"] == nil)
        #expect(items["official"] == nil)
    }

    @Test("categories decodes the data envelope")
    func categoriesDecodesEnvelope() async throws {
        let json = """
        {
            "data": [
                { "slug": "productivity", "name": "productivity", "count": 12 },
                { "slug": "git", "name": "git", "count": 5 }
            ]
        }
        """
        let service = makeService { _ in (200, Data(json.utf8)) }

        let categories = try await service.categories()

        #expect(categories.count == 2)
        #expect(categories.first?.slug == "productivity")
        #expect(categories.first?.count == 12)
    }

    @Test("resolveVersions decodes the flat version map and drops nulls")
    func resolveVersionsDecodesMap() async throws {
        let json = """
        { "git-status": "1.4.2", "dracula-theme": "0.9.1", "ghost": null }
        """
        let service = makeService { _ in (200, Data(json.utf8)) }

        let map = try await service.resolveVersions(names: ["git-status", "dracula-theme", "ghost"])

        #expect(map["git-status"] == "1.4.2")
        #expect(map["dracula-theme"] == "0.9.1")
        #expect(map["ghost"] == nil)
    }

    @Test("resolveVersions returns empty for no names without a request")
    func resolveVersionsEmpty() async throws {
        let service = makeService { _ in (500, Data()) }

        let map = try await service.resolveVersions(names: [])

        #expect(map.isEmpty)
    }

    @Test("maps 404 to notFound")
    func fetchMapsNotFound() async throws {
        let service = makeService { _ in (404, Data(#"{"message":"Not Found"}"#.utf8)) }

        await #expect(throws: MarketplaceError.notFound) {
            _ = try await service.fetch(name: "missing")
        }
    }

    @Test("fetch rejects a traversal name without issuing a request")
    func fetchRejectsTraversalName() async throws {
        let service = makeService { _ in (200, Data(#"{"data":{}}"#.utf8)) }

        await #expect(throws: MarketplaceError.notFound) {
            _ = try await service.fetch(name: "../../admin")
        }
    }

    @Test("list keeps decodable rows and applies defaults for missing fields")
    func listDefaultsMissingListingFields() async throws {
        let json = """
        {
            "data": [
                { "name": "minimal" },
                { "name": "full", "official": true, "downloads": 9, "version": "2.0.0", "categories": ["git"] }
            ],
            "links": { "next": null }
        }
        """
        let service = makeService { _ in (200, Data(json.utf8)) }

        let page = try await service.list(query: ExtensionCatalogQuery())

        #expect(page.items.count == 2)
        #expect(page.items.first?.official == false)
        #expect(page.items.first?.downloads == 0)
        #expect(page.items.first?.version == "0.0.0")
        #expect(page.items.first?.categories.isEmpty == true)
    }

    @Test("download rejects a download URL on a foreign host")
    func downloadRejectsForeignHost() async throws {
        let payload = Data("zip-bytes".utf8)
        let service = makeService { _ in (200, payload) }
        let ext = makeExtension(
            sha256: sha256Hex(payload),
            size: payload.count,
            downloadURL: "https://evil.example/download"
        )

        await #expect(throws: MarketplaceError.invalidArchive) {
            _ = try await service.download(ext)
        }
    }

    @Test("download rejects a non-https download URL")
    func downloadRejectsNonHTTPS() async throws {
        let payload = Data("zip-bytes".utf8)
        let service = makeService { _ in (200, payload) }
        let ext = makeExtension(
            sha256: sha256Hex(payload),
            size: payload.count,
            downloadURL: "file:///etc/passwd"
        )

        await #expect(throws: MarketplaceError.invalidArchive) {
            _ = try await service.download(ext)
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

    private func makeExtension(
        sha256: String,
        size: Int,
        downloadURL: String = "https://muxy.test/download"
    ) -> MarketplaceExtension {
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
            "download_url": "\(downloadURL)"
        }
        """
        return try! JSONDecoder().decode(MarketplaceExtension.self, from: Data(json.utf8))
    }

    private func sha256Hex(_ data: Data) -> String {
        SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    }

    private func queryItems(_ url: URL?) -> [String: String] {
        guard let url, let components = URLComponents(url: url, resolvingAgainstBaseURL: false) else {
            return [:]
        }
        var result: [String: String] = [:]
        for item in components.queryItems ?? [] {
            result[item.name] = item.value
        }
        return result
    }
}

private final class RequestBox: @unchecked Sendable {
    var url: URL?
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
