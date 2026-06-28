import Foundation

enum QuitConfirmationPreferences {
    static let confirmQuitKey = "muxy.app.confirmQuit"

    static var confirmQuit: Bool {
        get {
            let defaults = UserDefaults.standard
            if defaults.object(forKey: confirmQuitKey) == nil { return true }
            return defaults.bool(forKey: confirmQuitKey)
        }
        set {
            UserDefaults.standard.set(newValue, forKey: confirmQuitKey)
        }
    }
}
