import Foundation

enum TabCloseConfirmationPreferences {
    static let confirmRunningProcessKey = "muxy.tabs.confirmCloseRunningProcess"

    static var confirmRunningProcess: Bool {
        get {
            let defaults = UserDefaults.standard
            if defaults.object(forKey: confirmRunningProcessKey) == nil { return true }
            return defaults.bool(forKey: confirmRunningProcessKey)
        }
        set {
            UserDefaults.standard.set(newValue, forKey: confirmRunningProcessKey)
        }
    }
}
