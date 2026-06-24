import Foundation
import Testing

@testable import Muxy

@Suite("BrowserHistoryStore", .serialized)
@MainActor
struct BrowserHistoryStoreTests {
    private let profile = BrowserProfile.defaultID
    private let other = UUID()

    private func makeStore(initial: [BrowserHistoryEntry] = []) -> BrowserHistoryStore {
        BrowserHistoryStore(persistence: InMemoryBrowserHistoryPersistence(initial: initial))
    }

    private func url(_ value: String) -> URL {
        URL(string: value)!
    }

    @Test("records an http url")
    func recordsURL() {
        let store = makeStore()
        store.record(url: url("https://example.com"), title: "Example", profileID: profile)
        #expect(store.entries.count == 1)
        #expect(store.entries.first?.url == "https://example.com")
        #expect(store.entries.first?.title == "Example")
    }

    @Test("skips non-http schemes")
    func skipsNonHTTP() {
        let store = makeStore()
        store.record(url: url("about:blank"), title: nil, profileID: profile)
        store.record(url: url("file:///etc/hosts"), title: nil, profileID: profile)
        #expect(store.entries.isEmpty)
    }

    @Test("revisiting bumps visit count and moves to front")
    func dedupesAndBumps() {
        let store = makeStore()
        store.record(url: url("https://a.com"), title: nil, profileID: profile)
        store.record(url: url("https://b.com"), title: nil, profileID: profile)
        store.record(url: url("https://a.com"), title: "A", profileID: profile)
        #expect(store.entries.count == 2)
        #expect(store.entries.first?.url == "https://a.com")
        #expect(store.entries.first?.visitCount == 2)
        #expect(store.entries.first?.title == "A")
    }

    @Test("trims to max entries")
    func trimsToCap() {
        let store = makeStore()
        for index in 0 ..< (BrowserHistoryStore.maxEntries + 50) {
            store.record(url: url("https://site\(index).com"), title: nil, profileID: profile)
        }
        #expect(store.entries.count == BrowserHistoryStore.maxEntries)
    }

    @Test("suggestions filter by query and profile")
    func suggestionsFilter() {
        let store = makeStore()
        store.record(url: url("https://github.com"), title: "GitHub", profileID: profile)
        store.record(url: url("https://gitlab.com"), title: "GitLab", profileID: profile)
        store.record(url: url("https://example.com"), title: "Example", profileID: other)

        let results = store.suggestions(for: "git", profileID: profile)
        #expect(results.count == 2)
        #expect(results.allSatisfy { $0.url.contains("git") })
        #expect(!results.contains { $0.url.contains("example") })
    }

    @Test("empty query returns most recent for the profile")
    func suggestionsEmptyQuery() {
        let store = makeStore()
        store.record(url: url("https://a.com"), title: nil, profileID: profile)
        store.record(url: url("https://b.com"), title: nil, profileID: profile)
        let results = store.suggestions(for: "", profileID: profile)
        #expect(results.first?.url == "https://b.com")
    }

    @Test("suggestions rank by visit count then recency")
    func suggestionsRanking() {
        let store = makeStore()
        store.record(url: url("https://one.com"), title: nil, profileID: profile)
        store.record(url: url("https://two.com"), title: nil, profileID: profile)
        store.record(url: url("https://one.com"), title: nil, profileID: profile)
        let results = store.suggestions(for: ".com", profileID: profile)
        #expect(results.first?.url == "https://one.com")
    }

    @Test("suggestions are capped to the limit")
    func suggestionsLimit() {
        let store = makeStore()
        for index in 0 ..< 20 {
            store.record(url: url("https://match\(index).com"), title: nil, profileID: profile)
        }
        let results = store.suggestions(for: "match", profileID: profile, limit: 5)
        #expect(results.count == 5)
    }

    @Test("title matching is case insensitive")
    func matchesTitle() {
        let store = makeStore()
        store.record(url: url("https://x.test"), title: "My Dashboard", profileID: profile)
        let results = store.suggestions(for: "dashboard", profileID: profile)
        #expect(results.count == 1)
    }

    @Test("clear removes only the given profile's entries")
    func clearProfile() {
        let store = makeStore()
        store.record(url: url("https://a.com"), title: nil, profileID: profile)
        store.record(url: url("https://b.com"), title: nil, profileID: other)
        store.clear(profileID: profile)
        #expect(store.entries.count == 1)
        #expect(store.entries.first?.profileID == other)
    }

    @Test("saveImmediately persists without waiting for debounce")
    func saveImmediatelyPersistsCurrentEntries() {
        let persistence = RecordingBrowserHistoryPersistence()
        let store = BrowserHistoryStore(persistence: persistence)
        store.record(url: url("https://a.com"), title: nil, profileID: profile)

        store.saveImmediately()

        #expect(persistence.savedEntries.map(\.url) == ["https://a.com"])
    }
}

private final class RecordingBrowserHistoryPersistence: BrowserHistoryPersisting {
    private(set) var savedEntries: [BrowserHistoryEntry] = []

    func loadEntries() throws -> [BrowserHistoryEntry] {
        []
    }

    func saveEntries(_ entries: [BrowserHistoryEntry]) throws {
        savedEntries = entries
    }
}
