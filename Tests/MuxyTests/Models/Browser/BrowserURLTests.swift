import Foundation
import Testing

@testable import Muxy

@Suite("BrowserURL", .serialized)
struct BrowserURLTests {
    init() {
        BrowserPreferences.searchEngine = .google
    }

    @Test("full https url is preserved")
    func httpsURL() {
        let url = BrowserURL.resolve(from: "https://muxy.app/docs")
        #expect(url?.absoluteString == "https://muxy.app/docs")
    }

    @Test("http url is preserved")
    func httpURL() {
        let url = BrowserURL.resolve(from: "http://localhost:3000")
        #expect(url?.absoluteString == "http://localhost:3000")
    }

    @Test("bare host gets https scheme")
    func bareHost() {
        let url = BrowserURL.resolve(from: "github.com/manaflow-ai/cmux")
        #expect(url?.absoluteString == "https://github.com/manaflow-ai/cmux")
    }

    @Test("localhost with port is treated as a host")
    func localhostHost() {
        let url = BrowserURL.resolve(from: "localhost:8080")
        #expect(url?.absoluteString == "https://localhost:8080")
    }

    @Test("plain words become a google search")
    func searchFallback() {
        let url = BrowserURL.resolve(from: "swift concurrency")
        #expect(url?.host == "www.google.com")
        #expect(url?.path == "/search")
        #expect(url?.query?.contains("swift") == true)
    }

    @Test("single word without dot becomes a search")
    func singleWordSearch() {
        let url = BrowserURL.resolve(from: "muxy")
        #expect(url?.host == "www.google.com")
    }

    @Test("empty input resolves to nil")
    func emptyInput() {
        #expect(BrowserURL.resolve(from: "   ") == nil)
    }

    @Test("dangerous schemes never resolve to a navigable url", arguments: [
        "javascript:alert(1)",
        "data:text/html,<script>alert(1)</script>",
        "file:///etc/passwd",
        "vbscript:msgbox(1)",
    ])
    func dangerousSchemesBecomeSearch(input: String) {
        let url = BrowserURL.resolve(from: input)
        #expect(url?.host == "www.google.com")
        #expect(url.map(BrowserURL.isAllowed) == true)
    }

    @Test("configured engine drives the address-bar search fallback")
    func configuredEngineDrivesFallback() {
        defer { BrowserPreferences.searchEngine = .google }
        BrowserPreferences.searchEngine = .duckDuckGo
        #expect(BrowserURL.resolve(from: "swift concurrency")?.host == "duckduckgo.com")
    }
}

@Suite("BrowserPreferences", .serialized)
struct BrowserPreferencesTests {
    private let key = BrowserPreferences.openLinksInBuiltInBrowserKey

    @Test("defaults to false")
    func defaultsOff() {
        let original = UserDefaults.standard.object(forKey: key)
        defer { restore(original) }
        UserDefaults.standard.removeObject(forKey: key)
        #expect(BrowserPreferences.openLinksInBuiltInBrowser == false)
    }

    @Test("persists when set")
    func persists() {
        let original = UserDefaults.standard.object(forKey: key)
        defer { restore(original) }
        BrowserPreferences.openLinksInBuiltInBrowser = true
        #expect(BrowserPreferences.openLinksInBuiltInBrowser == true)
    }

    private func restore(_ original: Any?) {
        if let original {
            UserDefaults.standard.set(original, forKey: key)
        } else {
            UserDefaults.standard.removeObject(forKey: key)
        }
    }
}

@Suite("BrowserSearchEngine", .serialized)
struct BrowserSearchEngineTests {
    private let key = BrowserPreferences.searchEngineKey

    @Test("each engine builds a query url for its endpoint", arguments: BrowserSearchEngine.allCases)
    func searchURLPerEngine(engine: BrowserSearchEngine) {
        let url = engine.searchURL(for: "swift testing")
        #expect(url?.absoluteString.hasPrefix(engine.endpoint) == true)
        #expect(url?.query?.contains("swift%20testing") == true || url?.query?.contains("swift+testing") == true)
    }

    @Test("default search engine is google")
    func defaultIsGoogle() {
        let original = UserDefaults.standard.object(forKey: key)
        defer { restore(original) }
        UserDefaults.standard.removeObject(forKey: key)
        #expect(BrowserPreferences.searchEngine == .google)
    }

    private func restore(_ original: Any?) {
        if let original {
            UserDefaults.standard.set(original, forKey: key)
        } else {
            UserDefaults.standard.removeObject(forKey: key)
        }
    }
}

@Suite("BrowserHomePage", .serialized)
struct BrowserHomePageTests {
    private let key = BrowserPreferences.homePageURLKey

    @Test("default home page is blank")
    func defaultIsBlank() {
        let original = UserDefaults.standard.object(forKey: key)
        defer { restore(original) }
        UserDefaults.standard.removeObject(forKey: key)
        #expect(BrowserPreferences.homePageURLString == BrowserHomePage.blankURLString)
        #expect(BrowserURL.homeURL?.absoluteString == BrowserHomePage.blankURLString)
    }

    @Test("custom website becomes the home url")
    func customWebsite() {
        let original = UserDefaults.standard.object(forKey: key)
        defer { restore(original) }
        BrowserPreferences.homePageURLString = "example.com"
        #expect(BrowserURL.homeURL?.absoluteString == "https://example.com")
    }

    @Test("empty custom value falls back to blank")
    func emptyCustomFallsBack() {
        #expect(BrowserHomePage.resolvedURL(from: "   ")?.absoluteString == BrowserHomePage.blankURLString)
    }

    @Test("blank and empty values count as blank mode")
    func blankModeDetection() {
        #expect(BrowserHomePage.isBlankMode(BrowserHomePage.blankURLString))
        #expect(BrowserHomePage.isBlankMode(""))
        #expect(BrowserHomePage.isBlankMode("   "))
        #expect(!BrowserHomePage.isBlankMode("example.com"))
    }

    @Test("empty values normalize to blank")
    func normalization() {
        #expect(BrowserHomePage.normalized("  ") == BrowserHomePage.blankURLString)
        #expect(BrowserHomePage.normalized("  example.com ") == "example.com")
    }

    private func restore(_ original: Any?) {
        if let original {
            UserDefaults.standard.set(original, forKey: key)
        } else {
            UserDefaults.standard.removeObject(forKey: key)
        }
    }
}
