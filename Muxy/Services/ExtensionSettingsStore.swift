import Foundation

@MainActor
@Observable
final class ExtensionSettingsStore {
    static let shared = ExtensionSettingsStore()

    private let defaults: UserDefaults
    private var changeTick: Int = 0

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    static let keyPrefix = "muxy.ext."

    static func storageKey(extensionID: String, key: String) -> String {
        "\(keyPrefix)\(extensionID).\(key)"
    }

    func value(extensionID: String, key: String) -> ExtensionJSON? {
        _ = changeTick
        return readValue(extensionID: extensionID, key: key)
    }

    func setValue(_ value: ExtensionJSON?, extensionID: String, key: String) {
        let storageKey = Self.storageKey(extensionID: extensionID, key: key)
        guard let value else {
            defaults.removeObject(forKey: storageKey)
            changeTick &+= 1
            return
        }
        defaults.set(encodeForStorage(value), forKey: storageKey)
        changeTick &+= 1
    }

    func effectiveValue(extensionID: String, entry: ExtensionSettingEntry) -> ExtensionJSON? {
        if let override = value(extensionID: extensionID, key: entry.key) {
            return override
        }
        return entry.defaultValue
    }

    func clearAll(extensionID: String) {
        let prefix = "\(Self.keyPrefix)\(extensionID)."
        for key in defaults.dictionaryRepresentation().keys where key.hasPrefix(prefix) {
            defaults.removeObject(forKey: key)
        }
        changeTick &+= 1
    }

    private func readValue(extensionID: String, key: String) -> ExtensionJSON? {
        let storageKey = Self.storageKey(extensionID: extensionID, key: key)
        guard let raw = defaults.object(forKey: storageKey) else { return nil }
        return decodeFromStorage(raw)
    }

    private func encodeForStorage(_ value: ExtensionJSON) -> Any {
        switch value {
        case .null: NSNull()
        case let .bool(value): value
        case let .number(value): value
        case let .string(value): value
        case let .array(value): value.map(encodeForStorage)
        case let .object(value): value.mapValues(encodeForStorage)
        }
    }

    private func decodeFromStorage(_ raw: Any) -> ExtensionJSON {
        if raw is NSNull { return .null }
        if let value = raw as? Bool { return .bool(value) }
        if let value = raw as? Double { return .number(value) }
        if let value = raw as? Int { return .number(Double(value)) }
        if let value = raw as? NSNumber {
            if CFGetTypeID(value) == CFBooleanGetTypeID() { return .bool(value.boolValue) }
            return .number(value.doubleValue)
        }
        if let value = raw as? String { return .string(value) }
        if let value = raw as? [Any] { return .array(value.map(decodeFromStorage)) }
        if let value = raw as? [String: Any] { return .object(value.mapValues(decodeFromStorage)) }
        return .null
    }
}
