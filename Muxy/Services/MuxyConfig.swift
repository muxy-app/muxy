import Foundation

@MainActor @Observable
final class MuxyConfig {
    static let shared = MuxyConfig()

    let ghosttyConfigURL: URL

    private static let ghosttyConfigFilename = "ghostty.conf"
    private static let systemGhosttyConfigPath = NSHomeDirectory() + "/.config/ghostty/config"

    private init() {
        let dir = Self.appSupportDirectory()
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
        let entry = "\(key) = \(value)"
        var content = readGhosttyConfig()

        var lines = content.components(separatedBy: "\n")
        var replaced = false
        for (i, line) in lines.enumerated() {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            guard trimmed.hasPrefix(key) else { continue }
            let afterKey = trimmed.dropFirst(key.count).trimmingCharacters(in: .whitespaces)
            guard afterKey.hasPrefix("=") else { continue }
            lines[i] = entry
            replaced = true
            break
        }

        if !replaced {
            lines.insert(entry, at: 0)
        }

        content = lines.joined(separator: "\n")
        try? writeGhosttyConfig(content)
    }

    func configValue(for key: String) -> String? {
        let content = readGhosttyConfig()
        for line in content.components(separatedBy: .newlines) {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            guard trimmed.hasPrefix(key) else { continue }
            let afterKey = trimmed.dropFirst(key.count).trimmingCharacters(in: .whitespaces)
            guard afterKey.hasPrefix("=") else { continue }
            return afterKey.dropFirst().trimmingCharacters(in: .whitespaces)
        }
        return nil
    }

    private func seedFromSystemGhosttyIfNeeded() {
        guard !FileManager.default.fileExists(atPath: ghosttyConfigURL.path) else { return }

        guard FileManager.default.fileExists(atPath: Self.systemGhosttyConfigPath),
              let systemContent = try? String(contentsOfFile: Self.systemGhosttyConfigPath, encoding: .utf8) else {
            try? writeGhosttyConfig("")
            return
        }

        try? writeGhosttyConfig(systemContent)
    }

    private static func appSupportDirectory() -> URL {
        let appSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
        let dir = appSupport.appendingPathComponent("Muxy", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        return dir
    }

    private static func restrictFilePermissions(_ url: URL) {
        try? FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: url.path)
    }
}
