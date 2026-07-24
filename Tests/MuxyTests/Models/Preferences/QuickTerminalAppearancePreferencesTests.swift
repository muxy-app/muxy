import Foundation
import Testing

@testable import Muxy

@Suite("QuickTerminalAppearancePreferences")
struct QuickTerminalAppearancePreferencesTests {
    @Test("uses a focused glass default")
    func defaultAppearance() {
        let appearance = QuickTerminalAppearancePreferences.appearance(defaults: makeDefaults())

        #expect(appearance.transparency == 18)
        #expect(appearance.blurIntensity == 70)
        #expect(abs(appearance.backgroundOpacity - 0.82) < 0.000_1)
        #expect(abs(appearance.blurFraction - 0.7) < 0.000_1)
        #expect(appearance.showsBlur)
    }

    @Test("reads a stored appearance")
    func storedAppearance() {
        let defaults = makeDefaults()
        defaults.set(40, forKey: QuickTerminalAppearancePreferences.transparencyKey)
        defaults.set(82, forKey: QuickTerminalAppearancePreferences.blurIntensityKey)

        let appearance = QuickTerminalAppearancePreferences.appearance(defaults: defaults)

        #expect(appearance.transparency == 40)
        #expect(appearance.blurIntensity == 82)
        #expect(abs(appearance.backgroundOpacity - 0.6) < 0.000_1)
        #expect(abs(appearance.blurFraction - 0.82) < 0.000_1)
    }

    @Test("clamps stored appearance values")
    func validatesStoredAppearance() {
        let defaults = makeDefaults()
        defaults.set(90, forKey: QuickTerminalAppearancePreferences.transparencyKey)
        defaults.set(140, forKey: QuickTerminalAppearancePreferences.blurIntensityKey)

        let appearance = QuickTerminalAppearancePreferences.appearance(defaults: defaults)

        #expect(appearance.transparency == 55)
        #expect(appearance.blurIntensity == 100)
    }

    @Test("persists appearance values within range")
    func persistsAppearance() {
        let defaults = makeDefaults()

        QuickTerminalAppearancePreferences.setTransparency(30, defaults: defaults)
        QuickTerminalAppearancePreferences.setBlurIntensity(40, defaults: defaults)

        let appearance = QuickTerminalAppearancePreferences.appearance(defaults: defaults)
        #expect(appearance.transparency == 30)
        #expect(appearance.blurIntensity == 40)
    }

    @Test("clamps written appearance values to safe ranges")
    func clampsWrittenAppearance() {
        let defaults = makeDefaults()

        QuickTerminalAppearancePreferences.setTransparency(90, defaults: defaults)
        QuickTerminalAppearancePreferences.setBlurIntensity(-10, defaults: defaults)

        #expect(defaults.integer(forKey: QuickTerminalAppearancePreferences.transparencyKey) == 55)
        #expect(defaults.integer(forKey: QuickTerminalAppearancePreferences.blurIntensityKey) == 0)
    }

    @Test("Reduce Transparency resolves to an opaque unblurred surface")
    func reduceTransparencyFallback() {
        let appearance = QuickTerminalAppearance(transparency: 42, blurIntensity: 88)

        #expect(appearance.resolvingReduceTransparency(false) == appearance)
        #expect(appearance.resolvingReduceTransparency(true) == QuickTerminalAppearance(
            transparency: 0,
            blurIntensity: 0
        ))
    }

    @Test("blur is hidden without transparency or intensity")
    func blurVisibility() {
        #expect(!QuickTerminalAppearance(transparency: 0, blurIntensity: 100).showsBlur)
        #expect(!QuickTerminalAppearance(transparency: 30, blurIntensity: 0).showsBlur)
        #expect(QuickTerminalAppearance(transparency: 30, blurIntensity: 1).showsBlur)
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
        defaults.set(value, forKey: QuickTerminalAppearancePreferences.blurIntensityKey)

        #expect(QuickTerminalAppearancePreferences.migrateLegacyBlur(defaults: defaults))
        #expect(!QuickTerminalAppearancePreferences.migrateLegacyBlur(defaults: defaults))

        #expect(defaults.integer(forKey: QuickTerminalAppearancePreferences.blurIntensityKey) == expectedIntensity)
    }

    @Test("numeric blur migration preserves and clamps intensity")
    func numericBlurMigration() {
        let defaults = makeDefaults()
        defaults.set(130, forKey: QuickTerminalAppearancePreferences.blurIntensityKey)

        #expect(QuickTerminalAppearancePreferences.migrateLegacyBlur(defaults: defaults))
        #expect(!QuickTerminalAppearancePreferences.migrateLegacyBlur(defaults: defaults))

        #expect(defaults.integer(forKey: QuickTerminalAppearancePreferences.blurIntensityKey) == 100)
    }

    @Test("canonical numeric blur migration is a no-op")
    func canonicalNumericMigration() {
        let defaults = makeDefaults()
        defaults.set(70, forKey: QuickTerminalAppearancePreferences.blurIntensityKey)

        #expect(!QuickTerminalAppearancePreferences.migrateLegacyBlur(defaults: defaults))
        #expect(defaults.integer(forKey: QuickTerminalAppearancePreferences.blurIntensityKey) == 70)
    }

    @Test("reading blur intensity does not mutate legacy storage")
    func blurReadIsPure() {
        let defaults = makeDefaults()
        defaults.set("strong", forKey: QuickTerminalAppearancePreferences.blurIntensityKey)

        _ = QuickTerminalAppearancePreferences.blurIntensity(defaults: defaults)

        #expect(defaults.string(forKey: QuickTerminalAppearancePreferences.blurIntensityKey) == "strong")
    }

    private func makeDefaults() -> UserDefaults {
        let suiteName = "QuickTerminalAppearancePreferencesTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            fatalError("Unable to create isolated UserDefaults suite")
        }
        defaults.removePersistentDomain(forName: suiteName)
        return defaults
    }
}
