import Foundation

@MainActor
final class ExtensionEnabledStore {
    static let shared = ExtensionEnabledStore()

    private let defaults: UserDefaults
    private static let keyPrefix = "muxy.ext.enabled."

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    func override(extensionID: String) -> Bool? {
        let key = Self.storageKey(extensionID: extensionID)
        guard defaults.object(forKey: key) != nil else { return nil }
        return defaults.bool(forKey: key)
    }

    func setOverride(_ enabled: Bool, extensionID: String) {
        defaults.set(enabled, forKey: Self.storageKey(extensionID: extensionID))
    }

    func clearOverride(extensionID: String) {
        defaults.removeObject(forKey: Self.storageKey(extensionID: extensionID))
    }

    private static func storageKey(extensionID: String) -> String {
        "\(keyPrefix)\(extensionID)"
    }
}
