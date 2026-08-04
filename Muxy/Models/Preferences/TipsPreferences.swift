import Foundation

enum TipsPreferences {
    static let visibleKey = "muxy.tips.visible"
    static let defaultVisible = true

    static func isVisible(defaults: UserDefaults = .standard) -> Bool {
        guard defaults.object(forKey: visibleKey) != nil else { return defaultVisible }
        return defaults.bool(forKey: visibleKey)
    }
}
