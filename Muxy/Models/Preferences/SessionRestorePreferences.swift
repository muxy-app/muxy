import Foundation

enum SessionRestorePreferences {
    static let autoResumeKey = "muxy.sessionRestore.autoResume"
    static let autoResumeDefault = false

    static func autoResumeEnabled(defaults: UserDefaults = .standard) -> Bool {
        defaults.object(forKey: autoResumeKey) == nil
            ? autoResumeDefault
            : defaults.bool(forKey: autoResumeKey)
    }

    static func setAutoResumeEnabled(_ value: Bool, defaults: UserDefaults = .standard) {
        defaults.set(value, forKey: autoResumeKey)
    }
}
