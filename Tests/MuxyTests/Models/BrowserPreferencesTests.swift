import Foundation
import Testing

@testable import Muxy

@Suite("BrowserPreferences")
@MainActor
struct BrowserPreferencesTests {
    @Test("defaults: ephemeral data, auto-open dev server, fallback home URL")
    func defaults() {
        let defaults = UserDefaults.standard
        let snapshot = (
            defaults.object(forKey: BrowserPreferences.persistDataKey),
            defaults.object(forKey: BrowserPreferences.autoOpenDevServerKey),
            defaults.object(forKey: BrowserPreferences.homeURLKey)
        )
        defaults.removeObject(forKey: BrowserPreferences.persistDataKey)
        defaults.removeObject(forKey: BrowserPreferences.autoOpenDevServerKey)
        defaults.removeObject(forKey: BrowserPreferences.homeURLKey)
        defer {
            restore(snapshot.0, forKey: BrowserPreferences.persistDataKey)
            restore(snapshot.1, forKey: BrowserPreferences.autoOpenDevServerKey)
            restore(snapshot.2, forKey: BrowserPreferences.homeURLKey)
        }

        #expect(BrowserPreferences.persistData == false)
        #expect(BrowserPreferences.autoOpenDevServer == true)
        #expect(BrowserPreferences.homeURL == BrowserPreferences.defaultHomeURL)
    }

    @Test("homeURL falls back to default when stored value is blank")
    func blankHomeURLFallsBack() {
        let defaults = UserDefaults.standard
        let snapshot = defaults.object(forKey: BrowserPreferences.homeURLKey)
        defaults.set("   ", forKey: BrowserPreferences.homeURLKey)
        defer { restore(snapshot, forKey: BrowserPreferences.homeURLKey) }

        #expect(BrowserPreferences.homeURL == BrowserPreferences.defaultHomeURL)
    }

    @Test("homeURL returns trimmed override when set")
    func customHomeURLReturned() {
        let defaults = UserDefaults.standard
        let snapshot = defaults.object(forKey: BrowserPreferences.homeURLKey)
        defaults.set("  https://example.com/start  ", forKey: BrowserPreferences.homeURLKey)
        defer { restore(snapshot, forKey: BrowserPreferences.homeURLKey) }

        #expect(BrowserPreferences.homeURL == "https://example.com/start")
    }

    private func restore(_ value: Any?, forKey key: String) {
        if let value {
            UserDefaults.standard.set(value, forKey: key)
        } else {
            UserDefaults.standard.removeObject(forKey: key)
        }
    }
}
