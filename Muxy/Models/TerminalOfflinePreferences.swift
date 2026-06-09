import Foundation

enum TerminalOfflinePreferences {
    static let enabledKey = "muxy.terminalOffline.enabled"
    static let idleThresholdKey = "muxy.terminalOffline.idleThresholdSeconds"

    static let defaultIsEnabled = true
    static let defaultIdleThreshold: TimeInterval = 300

    static var isEnabled: Bool {
        get {
            let defaults = UserDefaults.standard
            if defaults.object(forKey: enabledKey) == nil { return defaultIsEnabled }
            return defaults.bool(forKey: enabledKey)
        }
        set { UserDefaults.standard.set(newValue, forKey: enabledKey) }
    }

    static var idleThreshold: TimeInterval {
        get {
            let defaults = UserDefaults.standard
            guard defaults.object(forKey: idleThresholdKey) != nil else { return defaultIdleThreshold }
            let stored = defaults.double(forKey: idleThresholdKey)
            return stored > 0 ? stored : defaultIdleThreshold
        }
        set { UserDefaults.standard.set(newValue, forKey: idleThresholdKey) }
    }
}
