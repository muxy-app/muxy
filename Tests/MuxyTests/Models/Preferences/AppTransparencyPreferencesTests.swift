import Foundation
import Testing

@testable import Muxy

@Suite("AppTransparencyPreferences")
struct AppTransparencyPreferencesTests {
    @Test("defaults to an opaque app background with vibrancy ready")
    func defaultAppearance() {
        let appearance = AppTransparencyPreferences.appearance(defaults: makeDefaults())

        #expect(appearance.transparency == 0)
        #expect(appearance.blurIntensity == 70)
        #expect(abs(appearance.backgroundOpacity - 1.0) < 0.000_1)
        #expect(!appearance.isTransparent)
        #expect(!appearance.showsBlur)
    }

    @Test("reads a stored appearance")
    func storedAppearance() {
        let defaults = makeDefaults()
        defaults.set(25, forKey: AppTransparencyPreferences.transparencyKey)
        defaults.set(80, forKey: AppTransparencyPreferences.blurIntensityKey)

        let appearance = AppTransparencyPreferences.appearance(defaults: defaults)

        #expect(appearance.transparency == 25)
        #expect(appearance.blurIntensity == 80)
        #expect(abs(appearance.backgroundOpacity - 0.75) < 0.000_1)
        #expect(abs(appearance.blurFraction - 0.8) < 0.000_1)
        #expect(appearance.isTransparent)
        #expect(appearance.showsBlur)
    }

    @Test("clamps stored appearance values")
    func validatesStoredAppearance() {
        let defaults = makeDefaults()
        defaults.set(90, forKey: AppTransparencyPreferences.transparencyKey)
        defaults.set(140, forKey: AppTransparencyPreferences.blurIntensityKey)

        let appearance = AppTransparencyPreferences.appearance(defaults: defaults)

        #expect(appearance.transparency == 55)
        #expect(appearance.blurIntensity == 100)
    }

    @Test("clamps written appearance values to safe ranges")
    func clampsWrittenAppearance() {
        let defaults = makeDefaults()

        AppTransparencyPreferences.setTransparency(90, defaults: defaults)
        AppTransparencyPreferences.setBlurIntensity(-10, defaults: defaults)

        #expect(defaults.integer(forKey: AppTransparencyPreferences.transparencyKey) == 55)
        #expect(defaults.integer(forKey: AppTransparencyPreferences.blurIntensityKey) == 0)
    }

    @Test("Reduce Transparency resolves to an opaque unblurred surface")
    func reduceTransparencyFallback() {
        let appearance = BackgroundAppearance(transparency: 42, blurIntensity: 88)

        #expect(appearance.resolvingReduceTransparency(false) == appearance)
        #expect(appearance.resolvingReduceTransparency(true) == BackgroundAppearance(
            transparency: 0,
            blurIntensity: 0
        ))
    }

    private func makeDefaults() -> UserDefaults {
        let suiteName = "AppTransparencyPreferencesTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            fatalError("Unable to create isolated UserDefaults suite")
        }
        defaults.removePersistentDomain(forName: suiteName)
        return defaults
    }
}
