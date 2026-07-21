import Foundation
import Testing

@testable import Muxy

@Suite("NotchTerminalSizePreferences")
struct NotchTerminalSizePreferencesTests {
    @Test("uses defaults when no size is stored")
    func defaultSize() {
        let defaults = makeDefaults()

        #expect(NotchTerminalSizePreferences.size(defaults: defaults).width == 720)
        #expect(NotchTerminalSizePreferences.size(defaults: defaults).height == 430)
    }

    @Test("reads a stored size")
    func storedSize() {
        let defaults = makeDefaults()
        defaults.set(960, forKey: NotchTerminalSizePreferences.widthKey)
        defaults.set(600, forKey: NotchTerminalSizePreferences.heightKey)

        #expect(NotchTerminalSizePreferences.size(defaults: defaults).width == 960)
        #expect(NotchTerminalSizePreferences.size(defaults: defaults).height == 600)
    }

    @Test("clamps stored values to safe ranges")
    func clampsStoredValues() {
        let defaults = makeDefaults()
        defaults.set(100, forKey: NotchTerminalSizePreferences.widthKey)
        defaults.set(2_000, forKey: NotchTerminalSizePreferences.heightKey)

        #expect(NotchTerminalSizePreferences.width(defaults: defaults) == 480)
        #expect(NotchTerminalSizePreferences.height(defaults: defaults) == 800)
    }

    private func makeDefaults() -> UserDefaults {
        let suiteName = "NotchTerminalSizePreferencesTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            fatalError("Unable to create isolated UserDefaults suite")
        }
        defaults.removePersistentDomain(forName: suiteName)
        return defaults
    }
}
