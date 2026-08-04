import Foundation
import Testing

@testable import Muxy

@Suite("TipsPreferences")
struct TipsPreferencesTests {
    @Test("tips are visible by default")
    func visibleByDefault() throws {
        let suiteName = "muxy.tests.\(UUID().uuidString)"
        let defaults = try #require(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }

        #expect(TipsPreferences.isVisible(defaults: defaults))
    }

    @Test("stored visibility overrides the default")
    func storedVisibility() throws {
        let suiteName = "muxy.tests.\(UUID().uuidString)"
        let defaults = try #require(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }
        defaults.set(false, forKey: TipsPreferences.visibleKey)

        #expect(!TipsPreferences.isVisible(defaults: defaults))
    }
}
