import Foundation

enum HomeProjectPreferences {
    static let visibleKey = "muxy.showHomeProject"
    static let defaultVisible = true

    static var isVisible: Bool {
        get {
            let defaults = UserDefaults.standard
            if defaults.object(forKey: visibleKey) == nil { return defaultVisible }
            return defaults.bool(forKey: visibleKey)
        }
        set { UserDefaults.standard.set(newValue, forKey: visibleKey) }
    }
}
