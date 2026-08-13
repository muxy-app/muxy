import Foundation
import Testing

@testable import Muxy

@Suite("Rich input preferences")
struct RichInputPreferencesTests {
    @Test("panel presentation is the default")
    func panelPresentationIsDefault() {
        #expect(RichInputPreferences.defaultPresentationMode == .panel)
        #expect(RichInputPreferences.defaultPosition == .right)
    }

    @Test("presentation modes have stable persisted values")
    func presentationModesHaveStablePersistedValues() {
        #expect(RichInputPresentationMode.allCases.map(\.rawValue) == ["panel", "floating"])
        #expect(RichInputPresentationMode.allCases.map(\.displayName) == ["Panel", "Floating"])
        #expect(RichInputPresentationMode(rawValue: "panel") == .panel)
        #expect(RichInputPresentationMode(rawValue: "floating") == .floating)
    }

    @Test("clear options default off with stable persisted keys")
    func clearOptionsDefaultOff() {
        #expect(RichInputPreferences.defaultClearAfterSending == false)
        #expect(RichInputPreferences.defaultClearOnClose == false)
        #expect(RichInputPreferences.clearAfterSendingKey == "muxy.richInput.clearAfterSending")
        #expect(RichInputPreferences.clearOnCloseKey == "muxy.richInput.clearOnClose")
    }

    @Test("reset removes persisted clear options")
    func resetRemovesPersistedClearOptions() throws {
        let suiteName = "RichInputPreferencesTests-\(UUID().uuidString)"
        let defaults = try #require(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }
        defaults.set(true, forKey: RichInputPreferences.clearAfterSendingKey)
        defaults.set(true, forKey: RichInputPreferences.clearOnCloseKey)

        RichInputPreferences.resetClearOptions(in: defaults)

        #expect(defaults.object(forKey: RichInputPreferences.clearAfterSendingKey) == nil)
        #expect(defaults.object(forKey: RichInputPreferences.clearOnCloseKey) == nil)
        #expect(defaults.bool(forKey: RichInputPreferences.clearAfterSendingKey) == false)
        #expect(defaults.bool(forKey: RichInputPreferences.clearOnCloseKey) == false)
    }
}
