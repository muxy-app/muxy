import Foundation

struct BackupManifest: Codable, Equatable {
    static let currentSchemaVersion = 1
    static let filename = "manifest.json"

    var schemaVersion: Int
    var appVersion: String
    var createdAt: Date
    var files: [String]

    var isSupported: Bool {
        schemaVersion >= 1 && schemaVersion <= Self.currentSchemaVersion
    }
}
