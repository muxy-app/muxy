import Foundation

struct CodexProvider: AIProviderIntegration {
    let id = "codex"
    let displayName = "Codex"
    let socketTypeKey = "codex_hook"
    let iconName = "codex"
    let executableNames = ["codex"]
    let hookScriptName = "muxy-codex-hook"

    private static let hooksPath = NSHomeDirectory() + "/.codex/hooks.json"
    private static let muxyMarker = "muxy-notification-hook"
    private static let installedEvents = ["Stop", "Notification"]

    func isToolInstalled() -> Bool {
        let home = NSHomeDirectory()
        let paths = [
            "\(home)/.local/bin/codex",
            "/usr/local/bin/codex",
            "/opt/homebrew/bin/codex",
        ]
        return paths.contains { FileManager.default.isExecutableFile(atPath: $0) }
    }

    func install(hookScriptPath: String) throws {
        let settings = try Self.readSettings()
        let hooks = settings["hooks"] as? [String: Any] ?? [:]

        var allMatch = true
        for event in Self.installedEvents {
            let expected = Self.hookCommand(hookScript: hookScriptPath, event: event.lowercased())
            if !Self.muxyHookMatches(entries: hooks[event] as? [[String: Any]], expectedCommand: expected) {
                allMatch = false
                break
            }
        }
        guard !allMatch else { return }

        var updatedSettings = settings
        var updatedHooks = hooks
        for event in Self.installedEvents {
            let command = Self.hookCommand(hookScript: hookScriptPath, event: event.lowercased())
            let entry = Self.buildHookEntry(command: command)
            updatedHooks[event] = Self.mergeHookArray(existing: hooks[event] as? [[String: Any]], muxyHook: entry)
        }
        updatedSettings["hooks"] = updatedHooks
        try Self.writeSettings(updatedSettings)
    }

    func uninstall() throws {
        guard FileManager.default.fileExists(atPath: Self.hooksPath) else { return }
        var settings = try Self.readSettings()
        guard var hooks = settings["hooks"] as? [String: Any] else { return }

        for key in Self.installedEvents {
            guard var entries = hooks[key] as? [[String: Any]] else { continue }
            entries.removeAll { Self.isMuxyHookEntry($0) }
            if entries.isEmpty {
                hooks.removeValue(forKey: key)
            } else {
                hooks[key] = entries
            }
        }

        settings["hooks"] = hooks
        try Self.writeSettings(settings)
    }

    private static func hookCommand(hookScript: String, event: String) -> String {
        "'\(hookScript)' \(event) # \(muxyMarker)"
    }

    private static func buildHookEntry(command: String) -> [String: Any] {
        [
            "hooks": [
                [
                    "type": "command",
                    "command": command,
                ] as [String: Any],
            ],
        ]
    }

    private static func muxyHookMatches(entries: [[String: Any]]?, expectedCommand: String) -> Bool {
        guard let entries else { return false }
        return entries.contains { entry in
            guard let hooks = entry["hooks"] as? [[String: Any]] else { return false }
            return hooks.contains { hook in
                guard let command = hook["command"] as? String else { return false }
                return command == expectedCommand
            }
        }
    }

    private static func mergeHookArray(
        existing: [[String: Any]]?,
        muxyHook: [String: Any]
    ) -> [[String: Any]] {
        var entries = existing ?? []
        entries.removeAll { isMuxyHookEntry($0) }
        entries.append(muxyHook)
        return entries
    }

    private static func isMuxyHookEntry(_ entry: [String: Any]) -> Bool {
        guard let hooks = entry["hooks"] as? [[String: Any]] else { return false }
        return hooks.contains { hook in
            guard let command = hook["command"] as? String else { return false }
            return command.contains(muxyMarker)
        }
    }

    private static func readSettings() throws -> [String: Any] {
        guard FileManager.default.fileExists(atPath: hooksPath) else { return [:] }
        let data = try Data(contentsOf: URL(fileURLWithPath: hooksPath))
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { return [:] }
        return json
    }

    private static func writeSettings(_ settings: [String: Any]) throws {
        let dirPath = (hooksPath as NSString).deletingLastPathComponent
        try FileManager.default.createDirectory(atPath: dirPath, withIntermediateDirectories: true)

        let fileURL = URL(fileURLWithPath: hooksPath)
        if FileManager.default.fileExists(atPath: hooksPath) {
            let backupPath = hooksPath + ".muxy-backup"
            let backupURL = URL(fileURLWithPath: backupPath)
            try? FileManager.default.removeItem(at: backupURL)
            try FileManager.default.copyItem(at: fileURL, to: backupURL)
        }

        let data = try JSONSerialization.data(withJSONObject: settings, options: [.prettyPrinted, .sortedKeys])
        try data.write(to: fileURL, options: .atomic)
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.privateFile],
            ofItemAtPath: hooksPath
        )
    }
}
