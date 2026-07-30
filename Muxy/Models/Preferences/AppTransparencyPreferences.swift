import Foundation

enum AppTransparencyPreferences {
    static let transparencyKey = "muxy.app.transparency"
    static let blurIntensityKey = "muxy.app.blur"
    static let defaultTransparency = 0
    static let defaultBlurIntensity = 70
    static let transparencyRange = BackgroundAppearance.transparencyRange
    static let blurIntensityRange = BackgroundAppearance.blurIntensityRange

    static func appearance(defaults: UserDefaults = .standard) -> BackgroundAppearance {
        BackgroundAppearance(
            transparency: transparency(defaults: defaults),
            blurIntensity: blurIntensity(defaults: defaults)
        )
    }

    static func transparency(defaults: UserDefaults = .standard) -> Int {
        guard defaults.object(forKey: transparencyKey) != nil else { return defaultTransparency }
        return min(
            max(defaults.integer(forKey: transparencyKey), transparencyRange.lowerBound),
            transparencyRange.upperBound
        )
    }

    static func blurIntensity(defaults: UserDefaults = .standard) -> Int {
        guard defaults.object(forKey: blurIntensityKey) != nil else { return defaultBlurIntensity }
        return min(
            max(defaults.integer(forKey: blurIntensityKey), blurIntensityRange.lowerBound),
            blurIntensityRange.upperBound
        )
    }

    static func setTransparency(_ value: Int, defaults: UserDefaults = .standard) {
        defaults.set(
            min(max(value, transparencyRange.lowerBound), transparencyRange.upperBound),
            forKey: transparencyKey
        )
    }

    static func setBlurIntensity(_ value: Int, defaults: UserDefaults = .standard) {
        defaults.set(
            min(max(value, blurIntensityRange.lowerBound), blurIntensityRange.upperBound),
            forKey: blurIntensityKey
        )
    }
}
