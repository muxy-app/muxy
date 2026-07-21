import Foundation
import Testing

@testable import Muxy

@Suite("NotchTerminalAppearancePreferences")
struct NotchTerminalAppearancePreferencesTests {
    @Test("uses a focused glass default")
    func defaultAppearance() {
        let appearance = NotchTerminalAppearancePreferences.appearance(defaults: makeDefaults())

        #expect(appearance.transparency == 18)
        #expect(appearance.blurIntensity == 70)
        #expect(abs(appearance.backgroundOpacity - 0.82) < 0.000_1)
        #expect(abs(appearance.blurFraction - 0.7) < 0.000_1)
        #expect(appearance.showsBlur)
    }

    @Test("reads a stored appearance")
    func storedAppearance() {
        let defaults = makeDefaults()
        defaults.set(40, forKey: NotchTerminalAppearancePreferences.transparencyKey)
        defaults.set(82, forKey: NotchTerminalAppearancePreferences.blurIntensityKey)

        let appearance = NotchTerminalAppearancePreferences.appearance(defaults: defaults)

        #expect(appearance.transparency == 40)
        #expect(appearance.blurIntensity == 82)
        #expect(abs(appearance.backgroundOpacity - 0.6) < 0.000_1)
        #expect(abs(appearance.blurFraction - 0.82) < 0.000_1)
    }

    @Test("clamps stored appearance values")
    func validatesStoredAppearance() {
        let defaults = makeDefaults()
        defaults.set(90, forKey: NotchTerminalAppearancePreferences.transparencyKey)
        defaults.set(140, forKey: NotchTerminalAppearancePreferences.blurIntensityKey)

        let appearance = NotchTerminalAppearancePreferences.appearance(defaults: defaults)

        #expect(appearance.transparency == 55)
        #expect(appearance.blurIntensity == 100)
    }

    @Test("Reduce Transparency resolves to an opaque unblurred surface")
    func reduceTransparencyFallback() {
        let appearance = NotchTerminalAppearance(transparency: 42, blurIntensity: 88)

        #expect(appearance.resolvingReduceTransparency(false) == appearance)
        #expect(appearance.resolvingReduceTransparency(true) == NotchTerminalAppearance(
            transparency: 0,
            blurIntensity: 0
        ))
    }

    @Test("blur is hidden without transparency or intensity")
    func blurVisibility() {
        #expect(!NotchTerminalAppearance(transparency: 0, blurIntensity: 100).showsBlur)
        #expect(!NotchTerminalAppearance(transparency: 30, blurIntensity: 0).showsBlur)
        #expect(NotchTerminalAppearance(transparency: 30, blurIntensity: 1).showsBlur)
    }

    @Test(arguments: [
        ("off", 0),
        ("light", 35),
        ("medium", 70),
        ("strong", 100),
        ("unknown", 70),
    ])
    func migratesLegacyBlur(value: String, expectedIntensity: Int) {
        let defaults = makeDefaults()
        defaults.set(value, forKey: NotchTerminalAppearancePreferences.blurIntensityKey)

        NotchTerminalAppearancePreferences.migrateLegacyBlur(defaults: defaults)
        NotchTerminalAppearancePreferences.migrateLegacyBlur(defaults: defaults)

        #expect(defaults.integer(forKey: NotchTerminalAppearancePreferences.blurIntensityKey) == expectedIntensity)
    }

    @Test("numeric blur migration preserves and clamps intensity")
    func numericBlurMigration() {
        let defaults = makeDefaults()
        defaults.set(130, forKey: NotchTerminalAppearancePreferences.blurIntensityKey)

        NotchTerminalAppearancePreferences.migrateLegacyBlur(defaults: defaults)

        #expect(defaults.integer(forKey: NotchTerminalAppearancePreferences.blurIntensityKey) == 100)
    }

    private func makeDefaults() -> UserDefaults {
        let suiteName = "NotchTerminalAppearancePreferencesTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            fatalError("Unable to create isolated UserDefaults suite")
        }
        defaults.removePersistentDomain(forName: suiteName)
        return defaults
    }
}
