import Foundation

enum TerminalPersistentSessionPreferences {
    static let enabledKey = "muxy.terminalPersistentSession.enabled"

    static let defaultIsEnabled = false

    static var isEnabled: Bool {
        get {
            let defaults = UserDefaults.standard
            guard defaults.object(forKey: enabledKey) != nil else { return defaultIsEnabled }
            return defaults.bool(forKey: enabledKey)
        }
        set { UserDefaults.standard.set(newValue, forKey: enabledKey) }
    }
}
