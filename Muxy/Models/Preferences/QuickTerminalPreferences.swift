import Foundation

enum QuickTerminalPreferences {
    static let enabledKey = "muxy.quickTerminal.enabled"
    static let defaultIsEnabled = true

    static func isEnabled(defaults: UserDefaults = .standard) -> Bool {
        guard defaults.object(forKey: enabledKey) != nil else { return defaultIsEnabled }
        return defaults.bool(forKey: enabledKey)
    }

    static func setEnabled(
        _ enabled: Bool,
        defaults: UserDefaults = .standard,
        notificationCenter: NotificationCenter = .default
    ) {
        guard isEnabled(defaults: defaults) != enabled else { return }
        defaults.set(enabled, forKey: enabledKey)
        notificationCenter.post(name: .quickTerminalEnabledDidChange, object: defaults)
    }
}
