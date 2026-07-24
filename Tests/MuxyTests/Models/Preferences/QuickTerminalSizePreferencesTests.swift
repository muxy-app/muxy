import Foundation
import Testing

@testable import Muxy

@Suite("QuickTerminalSizePreferences")
struct QuickTerminalSizePreferencesTests {
    @Test("uses defaults when no size is stored")
    func defaultSize() {
        let defaults = makeDefaults()

        #expect(QuickTerminalSizePreferences.size(defaults: defaults).width == 720)
        #expect(QuickTerminalSizePreferences.size(defaults: defaults).height == 430)
    }

    @Test("reads a stored size")
    func storedSize() {
        let defaults = makeDefaults()
        defaults.set(960, forKey: QuickTerminalSizePreferences.widthKey)
        defaults.set(600, forKey: QuickTerminalSizePreferences.heightKey)

        #expect(QuickTerminalSizePreferences.size(defaults: defaults).width == 960)
        #expect(QuickTerminalSizePreferences.size(defaults: defaults).height == 600)
    }

    @Test("clamps stored values to safe ranges")
    func clampsStoredValues() {
        let defaults = makeDefaults()
        defaults.set(100, forKey: QuickTerminalSizePreferences.widthKey)
        defaults.set(2_000, forKey: QuickTerminalSizePreferences.heightKey)

        #expect(QuickTerminalSizePreferences.width(defaults: defaults) == 480)
        #expect(QuickTerminalSizePreferences.height(defaults: defaults) == 800)
    }

    @Test("persists size values within range")
    func persistsSize() {
        let defaults = makeDefaults()

        QuickTerminalSizePreferences.setWidth(960, defaults: defaults)
        QuickTerminalSizePreferences.setHeight(600, defaults: defaults)

        #expect(QuickTerminalSizePreferences.width(defaults: defaults) == 960)
        #expect(QuickTerminalSizePreferences.height(defaults: defaults) == 600)
    }

    @Test("clamps written size values to safe ranges")
    func clampsWrittenSize() {
        let defaults = makeDefaults()

        QuickTerminalSizePreferences.setWidth(100, defaults: defaults)
        QuickTerminalSizePreferences.setHeight(5_000, defaults: defaults)

        #expect(defaults.integer(forKey: QuickTerminalSizePreferences.widthKey) == 480)
        #expect(defaults.integer(forKey: QuickTerminalSizePreferences.heightKey) == 800)
    }

    private func makeDefaults() -> UserDefaults {
        let suiteName = "QuickTerminalSizePreferencesTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            fatalError("Unable to create isolated UserDefaults suite")
        }
        defaults.removePersistentDomain(forName: suiteName)
        return defaults
    }
}
