import Foundation
import Testing

@testable import Muxy

@Suite("BrowserURLNormalizer")
struct BrowserURLNormalizerTests {
    @Test("preserves explicit scheme")
    func preservesExplicitScheme() {
        let url = BrowserURLNormalizer.normalize("https://example.com")
        #expect(url?.absoluteString == "https://example.com")
    }

    @Test("prepends http for localhost")
    func prependsHttpForLocalhost() {
        let url = BrowserURLNormalizer.normalize("localhost:3000")
        #expect(url?.absoluteString == "http://localhost:3000")
    }

    @Test("prepends http for ip address")
    func prependsHttpForIPAddress() {
        let url = BrowserURLNormalizer.normalize("127.0.0.1:8080")
        #expect(url?.absoluteString == "http://127.0.0.1:8080")
    }

    @Test("prepends http for domain")
    func prependsHttpForDomain() {
        let url = BrowserURLNormalizer.normalize("example.com")
        #expect(url?.absoluteString == "http://example.com")
    }

    @Test("falls back to search when input is plain text")
    func fallbackToSearch() {
        let url = BrowserURLNormalizer.normalize("hello world")
        #expect(url?.host == "duckduckgo.com")
        #expect(url?.absoluteString.contains("q=hello%20world") == true)
    }

    @Test("rejects empty input")
    func rejectsEmpty() {
        #expect(BrowserURLNormalizer.normalize("") == nil)
        #expect(BrowserURLNormalizer.normalize("   ") == nil)
    }
}
