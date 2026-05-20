import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "MuxyConfig")

@MainActor @Observable
final class MuxyConfig {
    static let shared = MuxyConfig()

    let ghosttyConfigURL: URL

    private static let ghosttyConfigFilename = "ghostty.conf"
    private static let systemGhosttyConfigPath = NSHomeDirectory() + "/.config/ghostty/config"

    private init() {
        let dir = MuxyFileStorage.appSupportDirectory()
        ghosttyConfigURL = dir.appendingPathComponent(Self.ghosttyConfigFilename)
        seedFromSystemGhosttyIfNeeded()
    }

    var ghosttyConfigPath: String {
        ghosttyConfigURL.path
    }

    func readGhosttyConfig() -> String {
        (try? String(contentsOf: ghosttyConfigURL, encoding: .utf8)) ?? ""
    }

    func writeGhosttyConfig(_ content: String) throws {
        let data = Data(content.utf8)
        try data.write(to: ghosttyConfigURL, options: .atomic)
        Self.restrictFilePermissions(ghosttyConfigURL)
    }

    func updateConfigValue(_ key: String, value: String) {
        writeUpdatedGhosttyConfig(GhosttyConfigFile.settingValue(value, for: key, in: readGhosttyConfig()))
    }

    func removeConfigValue(_ key: String) {
        writeUpdatedGhosttyConfig(GhosttyConfigFile.removingValue(for: key, in: readGhosttyConfig()))
    }

    func configValue(for key: String) -> String? {
        GhosttyConfigFile.value(for: key, in: readGhosttyConfig())
    }

    private func writeUpdatedGhosttyConfig(_ content: String) {
        do {
            try writeGhosttyConfig(content)
        } catch {
            logger.error("Failed to write config: \(error)")
        }
    }

    private func seedFromSystemGhosttyIfNeeded() {
        guard !FileManager.default.fileExists(atPath: ghosttyConfigURL.path) else { return }

        guard FileManager.default.fileExists(atPath: Self.systemGhosttyConfigPath),
              let systemContent = try? String(contentsOfFile: Self.systemGhosttyConfigPath, encoding: .utf8)
        else {
            try? writeGhosttyConfig("")
            return
        }

        try? writeGhosttyConfig(systemContent)
    }

    private static func restrictFilePermissions(_ url: URL) {
        try? FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.privateFile],
            ofItemAtPath: url.path
        )
    }
}
