import Foundation

enum ExtensionEnabledStore {
    private static let keyPrefix = "muxy.ext.enabled."

    static func isEnabled(extensionID: String, defaults: UserDefaults = .standard) -> Bool {
        let key = storageKey(extensionID: extensionID)
        guard defaults.object(forKey: key) != nil else { return true }
        return defaults.bool(forKey: key)
    }

    static func hasOverride(extensionID: String, defaults: UserDefaults = .standard) -> Bool {
        defaults.object(forKey: storageKey(extensionID: extensionID)) != nil
    }

    static func setEnabled(_ enabled: Bool, extensionID: String, defaults: UserDefaults = .standard) {
        defaults.set(enabled, forKey: storageKey(extensionID: extensionID))
    }

    static func clear(extensionID: String, defaults: UserDefaults = .standard) {
        defaults.removeObject(forKey: storageKey(extensionID: extensionID))
    }

    private static func storageKey(extensionID: String) -> String {
        "\(keyPrefix)\(extensionID)"
    }
}
