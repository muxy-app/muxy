import Foundation

enum BrowserPreferences {
    static let persistDataKey = "muxy.browser.persistData"
    static let defaultPersistData = false

    static let autoOpenDevServerKey = "muxy.browser.autoOpenDevServer"
    static let defaultAutoOpenDevServer = true

    static let homeURLKey = "muxy.browser.homeURL"
    static let defaultHomeURL = "https://www.google.com"

    static let inspectableKey = "muxy.browser.inspectable"
    static var defaultInspectable: Bool {
        #if DEBUG
        true
        #else
        false
        #endif
    }

    static var persistData: Bool {
        get { boolValue(forKey: persistDataKey, defaultValue: defaultPersistData) }
        set { UserDefaults.standard.set(newValue, forKey: persistDataKey) }
    }

    static var autoOpenDevServer: Bool {
        get { boolValue(forKey: autoOpenDevServerKey, defaultValue: defaultAutoOpenDevServer) }
        set { UserDefaults.standard.set(newValue, forKey: autoOpenDevServerKey) }
    }

    static var homeURL: String {
        let stored = UserDefaults.standard.string(forKey: homeURLKey)?
            .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        return stored.isEmpty ? defaultHomeURL : stored
    }

    static var inspectable: Bool {
        get { boolValue(forKey: inspectableKey, defaultValue: defaultInspectable) }
        set { UserDefaults.standard.set(newValue, forKey: inspectableKey) }
    }

    private static func boolValue(forKey key: String, defaultValue: Bool) -> Bool {
        let defaults = UserDefaults.standard
        if defaults.object(forKey: key) == nil { return defaultValue }
        return defaults.bool(forKey: key)
    }
}
