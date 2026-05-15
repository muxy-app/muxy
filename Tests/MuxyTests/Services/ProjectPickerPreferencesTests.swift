import Foundation
import Testing

@testable import Muxy

@Suite("ProjectPickerPreferences")
struct ProjectPickerPreferencesTests {
    @Test("custom picker is the default and the selected picker persists")
    func pickerModePersists() throws {
        let suiteName = "ProjectPickerPreferencesTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            Issue.record("Unable to create isolated UserDefaults suite")
            return
        }
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let preferences = ProjectPickerPreferences(defaults: defaults)

        #expect(preferences.mode == .custom)

        preferences.mode = .finder

        #expect(ProjectPickerPreferences(defaults: defaults).mode == .finder)
    }
}
