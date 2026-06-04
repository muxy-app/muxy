import CoreGraphics
import Foundation

enum TabWidthPreferences {
    static let maxWidthKey = "muxy.tabs.maxWidth"
    static let fullWidthValue: Double = 0
    static let smallMaxWidth: Double = 200
    static let mediumMaxWidth: Double = 400

    static var defaultMaxWidth: Double { fullWidthValue }

    enum HeaderSize: String, CaseIterable, Identifiable {
        case small
        case medium
        case fullWidth = "full-width"

        var id: String { rawValue }
        var title: String {
            switch self {
            case .small: "Small"
            case .medium: "Medium"
            case .fullWidth: "Full-width"
            }
        }

        var storedMaxWidth: Double? {
            switch self {
            case .small: TabWidthPreferences.smallMaxWidth
            case .medium: TabWidthPreferences.mediumMaxWidth
            case .fullWidth: nil
            }
        }

        var jsonValue: Double {
            storedMaxWidth ?? TabWidthPreferences.fullWidthValue
        }
    }

    static func effectiveMaxWidth(from storedValue: Double) -> CGFloat? {
        guard storedValue > 0 else { return nil }
        return CGFloat(storedValue)
    }

    static func headerSize(from storedValue: Double) -> HeaderSize {
        if storedValue == smallMaxWidth { return .small }
        if storedValue == mediumMaxWidth { return .medium }
        return .fullWidth
    }

    static func currentHeaderSize(defaults: UserDefaults = .standard) -> HeaderSize {
        guard let number = defaults.object(forKey: maxWidthKey) as? NSNumber else {
            return .fullWidth
        }
        return headerSize(from: number.doubleValue)
    }

    static func store(_ headerSize: HeaderSize, defaults: UserDefaults = .standard) {
        guard let width = headerSize.storedMaxWidth else {
            defaults.removeObject(forKey: maxWidthKey)
            return
        }
        defaults.set(width, forKey: maxWidthKey)
    }

    static func isAllowedStoredValue(_ value: Double) -> Bool {
        value >= 0 && value.isFinite
    }
}
