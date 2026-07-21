import AppKit

enum QuickTerminalSizePreferences {
    static let widthKey = "muxy.quickTerminal.width"
    static let heightKey = "muxy.quickTerminal.height"
    static let defaultWidth = 720
    static let defaultHeight = 430
    static let widthRange = 480 ... 1200
    static let heightRange = 280 ... 800

    static func size(defaults: UserDefaults = .standard) -> NSSize {
        NSSize(
            width: CGFloat(width(defaults: defaults)),
            height: CGFloat(height(defaults: defaults))
        )
    }

    static func width(defaults: UserDefaults = .standard) -> Int {
        storedValue(forKey: widthKey, defaultValue: defaultWidth, range: widthRange, defaults: defaults)
    }

    static func height(defaults: UserDefaults = .standard) -> Int {
        storedValue(forKey: heightKey, defaultValue: defaultHeight, range: heightRange, defaults: defaults)
    }

    static func setWidth(_ value: Int, defaults: UserDefaults = .standard) {
        defaults.set(min(max(value, widthRange.lowerBound), widthRange.upperBound), forKey: widthKey)
    }

    static func setHeight(_ value: Int, defaults: UserDefaults = .standard) {
        defaults.set(min(max(value, heightRange.lowerBound), heightRange.upperBound), forKey: heightKey)
    }

    private static func storedValue(
        forKey key: String,
        defaultValue: Int,
        range: ClosedRange<Int>,
        defaults: UserDefaults
    ) -> Int {
        guard defaults.object(forKey: key) != nil else { return defaultValue }
        return min(max(defaults.integer(forKey: key), range.lowerBound), range.upperBound)
    }
}
