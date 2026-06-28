import CoreGraphics
import Foundation

enum TabWidthPreferences {
    static let maxWidthKey = "muxy.tabs.maxWidth"
    static let minMaxWidth: Double = 120
    static let maxMaxWidth: Double = 400
    static let defaultMaxWidth: Double = 200

    static func effectiveMaxWidth(from storedValue: Double) -> CGFloat? {
        guard storedValue > 0, storedValue < maxMaxWidth else { return nil }
        return CGFloat(storedValue)
    }

    static func sliderValue(from storedValue: Double) -> Double {
        guard storedValue > 0, storedValue < maxMaxWidth else { return maxMaxWidth }
        return min(max(storedValue, minMaxWidth), maxMaxWidth)
    }

    static func storedValue(forSlider sliderValue: Double) -> Double {
        sliderValue >= maxMaxWidth ? 0 : min(max(sliderValue, minMaxWidth), maxMaxWidth)
    }

    static func isAllowedStoredValue(_ value: Double) -> Bool {
        value >= 0 && value.isFinite
    }
}
