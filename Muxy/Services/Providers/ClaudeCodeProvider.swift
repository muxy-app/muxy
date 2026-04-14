import Foundation

struct ClaudeCodeProvider: AIProviderIntegration {
    let id = "claude_code"
    let displayName = "Claude Code"
    let socketTypeKey = "claude_hook"
    let iconName = "sparkles"
    let executableNames = ["claude"]

    private static let settingsPath = NSHomeDirectory() + "/.claude/settings.json"
    private static let muxyMarker = "muxy-notification-hook"

    func isToolInstalled() -> Bool {
        let home = NSHomeDirectory()
        let paths = [
            "\(home)/.local/bin/claude",
            "/usr/local/bin/claude",
            "/opt/homebrew/bin/claude",
        ]
        return paths.contains { FileManager.default.isExecutableFile(atPath: $0) }
    }

    func install(hookScriptPath: String) throws {
        var settings = try Self.readSettings()
        var hooks = settings["hooks"] as? [String: Any] ?? [:]

        let stopHook = Self.buildHookEntry(hookScript: hookScriptPath, event: "stop")
        let notificationHook = Self.buildHookEntry(hookScript: hookScriptPath, event: "notification")

        hooks["Stop"] = Self.mergeHookArray(existing: hooks["Stop"] as? [[String: Any]], muxyHook: stopHook)
        hooks["Notification"] = Self.mergeHookArray(
            existing: hooks["Notification"] as? [[String: Any]],
            muxyHook: notificationHook
        )

        settings["hooks"] = hooks
        try Self.writeSettings(settings)
    }

    func uninstall() throws {
        guard FileManager.default.fileExists(atPath: Self.settingsPath) else { return }
        var settings = try Self.readSettings()
        guard var hooks = settings["hooks"] as? [String: Any] else { return }

        for key in ["Stop", "Notification"] {
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

    private static func buildHookEntry(hookScript: String, event: String) -> [String: Any] {
        [
            "matcher": "",
            "hooks": [
                [
                    "type": "command",
                    "command": "'\(hookScript)' \(event) # \(muxyMarker)",
                    "timeout": 10,
                ] as [String: Any],
            ],
        ]
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
        guard FileManager.default.fileExists(atPath: settingsPath) else { return [:] }
        let data = try Data(contentsOf: URL(fileURLWithPath: settingsPath))
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { return [:] }
        return json
    }

    private static func writeSettings(_ settings: [String: Any]) throws {
        let dirPath = (settingsPath as NSString).deletingLastPathComponent
        try FileManager.default.createDirectory(atPath: dirPath, withIntermediateDirectories: true)
        let data = try JSONSerialization.data(withJSONObject: settings, options: [.prettyPrinted, .sortedKeys])
        try data.write(to: URL(fileURLWithPath: settingsPath), options: .atomic)
    }
}
