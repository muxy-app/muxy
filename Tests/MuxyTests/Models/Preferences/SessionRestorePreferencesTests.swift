import Foundation
import Testing

@testable import Muxy

@Suite("SessionRestorePreferences")
struct SessionRestorePreferencesTests {
    @Test("defaults to disabled")
    func defaultDisabled() {
        #expect(SessionRestorePreferences.autoResumeEnabled(defaults: makeDefaults()) == false)
    }

    @Test("persists an enabled value")
    func persists() {
        let defaults = makeDefaults()
        SessionRestorePreferences.setAutoResumeEnabled(true, defaults: defaults)
        #expect(SessionRestorePreferences.autoResumeEnabled(defaults: defaults) == true)
    }

    private func makeDefaults() -> UserDefaults {
        let suiteName = "SessionRestorePreferencesTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            fatalError("Unable to create isolated UserDefaults suite")
        }
        defaults.removePersistentDomain(forName: suiteName)
        return defaults
    }
}
