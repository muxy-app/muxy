import CryptoKit
import Foundation

@MainActor
enum ExtensionStorageService {
    static let maxKeyLength = 256
    static let maxValueBytes = 1_048_576
    static let maxStoreBytes = 5_242_880

    static func get(extensionID: String, key: String) throws -> Any {
        let key = try validatedKey(key)
        let store = try load(extensionID)
        return store[key] ?? NSNull()
    }

    static func set(extensionID: String, key: String, value: Any) throws {
        let key = try validatedKey(key)
        try validate(value: value)
        var store = try load(extensionID)
        store[key] = value
        try save(extensionID, store: store)
    }

    static func delete(extensionID: String, key: String) throws {
        let key = try validatedKey(key)
        var store = try load(extensionID)
        guard store.removeValue(forKey: key) != nil else { return }
        try save(extensionID, store: store)
    }

    static func keys(extensionID: String) throws -> [String] {
        try load(extensionID).keys.sorted()
    }

    private static func validatedKey(_ key: String) throws -> String {
        guard !key.isEmpty else { throw APIError.invalidArguments("storage key must not be empty") }
        guard key.count <= maxKeyLength else {
            throw APIError.invalidArguments("storage key exceeds \(maxKeyLength) characters")
        }
        return key
    }

    private static func validate(value: Any) throws {
        guard JSONSerialization.isValidJSONObject([value]) else {
            throw APIError.invalidArguments("storage value is not JSON-serializable")
        }
        let data = try? JSONSerialization.data(withJSONObject: value, options: [.fragmentsAllowed])
        guard let data else { throw APIError.invalidArguments("storage value is not JSON-serializable") }
        guard data.count <= maxValueBytes else {
            throw APIError.invalidArguments("storage value exceeds \(maxValueBytes) bytes")
        }
    }

    private static func load(_ extensionID: String) throws -> [String: Any] {
        let url = try fileURL(extensionID)
        guard let data = try? Data(contentsOf: url), !data.isEmpty else { return [:] }
        guard let object = try? JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed]),
              let store = object as? [String: Any]
        else { return [:] }
        return store
    }

    private static func save(_ extensionID: String, store: [String: Any]) throws {
        let url = try fileURL(extensionID)
        let data = try JSONSerialization.data(withJSONObject: store, options: [.sortedKeys])
        guard data.count <= maxStoreBytes else {
            throw APIError.invalidArguments("storage for this extension exceeds \(maxStoreBytes) bytes")
        }
        try data.write(to: url, options: [.atomic])
        try? FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.privateFile],
            ofItemAtPath: url.path
        )
    }

    private static func fileURL(_ extensionID: String) throws -> URL {
        let directory = MuxyFileStorage.appSupportDirectory()
            .appendingPathComponent("extension-storage", isDirectory: true)
        try FileManager.default.createDirectory(
            at: directory,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: FilePermissions.privateDirectory]
        )
        return directory.appendingPathComponent("\(safeFilename(extensionID)).json")
    }

    private static func safeFilename(_ extensionID: String) -> String {
        let allowed = CharacterSet(charactersIn: "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_.")
        let slug = String(extensionID.unicodeScalars.prefix(64).map { allowed.contains($0) ? Character($0) : "_" })
        let digest = SHA256.hash(data: Data(extensionID.utf8))
        let suffix = digest.prefix(8).map { String(format: "%02x", $0) }.joined()
        return "\(slug.isEmpty ? "_" : slug)-\(suffix)"
    }
}
