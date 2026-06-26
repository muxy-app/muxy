import Foundation
import Testing

@testable import Muxy

@Suite("FaviconStore")
struct FaviconStoreTests {
    @Test("same host produces the same key regardless of path")
    func sameHostSameKey() throws {
        let a = try #require(URL(string: "https://muxy.app/docs"))
        let b = try #require(URL(string: "https://muxy.app/blog/post"))
        #expect(FaviconStore.cacheKey(for: a) == FaviconStore.cacheKey(for: b))
    }

    @Test("different hosts produce different keys")
    func differentHostsDifferentKeys() throws {
        let a = try #require(URL(string: "https://muxy.app"))
        let b = try #require(URL(string: "https://example.com"))
        #expect(FaviconStore.cacheKey(for: a) != FaviconStore.cacheKey(for: b))
    }

    @Test("hostless url falls back to absolute string")
    func hostlessFallback() throws {
        let url = try #require(URL(string: "about:blank"))
        #expect(FaviconStore.cacheKey(for: url) == "about:blank")
    }
}
