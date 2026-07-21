import Foundation

struct QuickTerminalAppearance: Equatable {
    let transparency: Int
    let blurIntensity: Int

    init(transparency: Int, blurIntensity: Int) {
        self.transparency = min(
            max(transparency, QuickTerminalAppearancePreferences.transparencyRange.lowerBound),
            QuickTerminalAppearancePreferences.transparencyRange.upperBound
        )
        self.blurIntensity = min(
            max(blurIntensity, QuickTerminalAppearancePreferences.blurIntensityRange.lowerBound),
            QuickTerminalAppearancePreferences.blurIntensityRange.upperBound
        )
    }

    var backgroundOpacity: Double {
        Double(100 - transparency) / 100
    }

    var blurFraction: Double {
        Double(blurIntensity) / 100
    }

    var showsBlur: Bool {
        transparency > 0 && blurIntensity > 0
    }

    func resolvingReduceTransparency(_ reduceTransparency: Bool) -> Self {
        guard reduceTransparency else { return self }
        return Self(transparency: 0, blurIntensity: 0)
    }
}

enum QuickTerminalAppearancePreferences {
    static let transparencyKey = "muxy.quickTerminal.transparency"
    static let blurIntensityKey = "muxy.quickTerminal.blur"
    static let defaultTransparency = 18
    static let defaultBlurIntensity = 70
    static let transparencyRange = 0 ... 55
    static let blurIntensityRange = 0 ... 100

    static func appearance(defaults: UserDefaults = .standard) -> QuickTerminalAppearance {
        QuickTerminalAppearance(
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

    @discardableResult
    static func migrateLegacyBlur(defaults: UserDefaults = .standard) -> Bool {
        guard let storedValue = defaults.object(forKey: blurIntensityKey) else { return false }
        if let number = storedValue as? NSNumber,
           CFGetTypeID(number) != CFBooleanGetTypeID()
        {
            let intensity = min(
                max(number.intValue, blurIntensityRange.lowerBound),
                blurIntensityRange.upperBound
            )
            guard storedValue as? Int != intensity else { return false }
            defaults.set(intensity, forKey: blurIntensityKey)
            return true
        }
        let intensity = switch storedValue as? String {
        case "off": 0
        case "light": 35
        case "medium": 70
        case "strong": 100
        default: defaultBlurIntensity
        }
        defaults.set(intensity, forKey: blurIntensityKey)
        return true
    }
}
