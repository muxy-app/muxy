import Foundation

enum BackgroundActivityPreferences {
    static let keepActiveKey = "muxy.terminal.keepActiveInBackground"
    static let defaultKeepActive = true

    static func keepActive(defaults: UserDefaults = .standard) -> Bool {
        guard defaults.object(forKey: keepActiveKey) != nil else { return defaultKeepActive }
        return defaults.bool(forKey: keepActiveKey)
    }

    static func setKeepActive(
        _ keepActive: Bool,
        defaults: UserDefaults = .standard,
        notificationCenter: NotificationCenter = .default
    ) {
        guard self.keepActive(defaults: defaults) != keepActive else { return }
        defaults.set(keepActive, forKey: keepActiveKey)
        notificationCenter.post(name: .backgroundActivityKeepActiveDidChange, object: defaults)
    }

    static func effectiveVisibility(
        keepActive: Bool,
        isPaneVisible: Bool,
        isWindowVisible: Bool
    ) -> Bool {
        keepActive || (isPaneVisible && isWindowVisible)
    }
}
