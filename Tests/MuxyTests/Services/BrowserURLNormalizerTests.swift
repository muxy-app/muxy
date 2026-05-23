import Foundation
import Testing

@testable import Muxy

@Suite("BrowserURLNormalizer")
struct BrowserURLNormalizerTests {
    @Test("preserves explicit https scheme")
    func preservesHTTPSScheme() {
        let url = BrowserURLNormalizer.normalize("https://example.com")
        #expect(url?.absoluteString == "https://example.com")
    }

    @Test("preserves explicit http scheme")
    func preservesHTTPScheme() {
        let url = BrowserURLNormalizer.normalize("http://example.com")
        #expect(url?.absoluteString == "http://example.com")
    }

    @Test("preserves about:blank")
    func preservesAboutBlank() {
        let url = BrowserURLNormalizer.normalize("about:blank")
        #expect(url?.absoluteString == "about:blank")
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
        #expect(url?.host == "www.google.com")
        #expect(url?.absoluteString.contains("q=hello%20world") == true)
    }

    @Test("rejects empty input")
    func rejectsEmpty() {
        #expect(BrowserURLNormalizer.normalize("") == nil)
        #expect(BrowserURLNormalizer.normalize("   ") == nil)
    }

    @Test("rejects javascript scheme")
    func rejectsJavaScriptScheme() {
        #expect(BrowserURLNormalizer.normalize("javascript:alert(1)") == nil)
        #expect(BrowserURLNormalizer.normalize("JavaScript:alert(1)") == nil)
    }

    @Test("rejects data scheme")
    func rejectsDataScheme() {
        #expect(BrowserURLNormalizer.normalize("data:text/html,<script>alert(1)</script>") == nil)
    }

    @Test("rejects file scheme")
    func rejectsFileScheme() {
        #expect(BrowserURLNormalizer.normalize("file:///etc/passwd") == nil)
    }

    @Test("isAllowedNavigationURL only permits web schemes and about:blank")
    func navigationAllowlist() {
        guard let https = URL(string: "https://example.com"),
              let http = URL(string: "http://example.com"),
              let blank = URL(string: "about:blank"),
              let file = URL(string: "file:///etc/passwd"),
              let dataURL = URL(string: "data:text/html,abc"),
              let javascript = URL(string: "javascript:alert(1)")
        else {
            Issue.record("Failed to build URLs")
            return
        }
        #expect(BrowserURLNormalizer.isAllowedNavigationURL(https))
        #expect(BrowserURLNormalizer.isAllowedNavigationURL(http))
        #expect(BrowserURLNormalizer.isAllowedNavigationURL(blank))
        #expect(!BrowserURLNormalizer.isAllowedNavigationURL(file))
        #expect(!BrowserURLNormalizer.isAllowedNavigationURL(dataURL))
        #expect(!BrowserURLNormalizer.isAllowedNavigationURL(javascript))
    }
}
