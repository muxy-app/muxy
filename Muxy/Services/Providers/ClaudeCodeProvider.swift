import Foundation

struct ClaudeCodeProvider: AIProviderIntegration, AIAgentLaunchProvider {
    let id = "claude"
    let displayName = "Claude Code"
    let socketTypeKey = "claude_hook"
    let iconName = "claude"
    let executableNames = ["claude"]

    var agentLaunchConfiguration: AIAgentLaunchConfiguration {
        AIAgentLaunchConfiguration(
            executable: "claude",
            headlessArguments: [
                "--print",
                "--output-format",
                "text",
                "--permission-mode",
                "dontAsk",
                "--no-session-persistence",
                "--tools=",
            ]
        )
    }

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

    static let hookEvents: [(settingsKey: String, event: String)] = [
        ("Stop", "stop"),
        ("StopFailure", "stop-failure"),
        ("SessionEnd", "session-end"),
        ("Notification", "notification"),
        ("PermissionRequest", "permission-request"),
        ("UserPromptSubmit", "user-prompt-submit"),
        ("PreToolUse", "pre-tool-use"),
    ]

    func install(hookScriptPath: String) throws {
        let settings = try Self.readSettings()
        let hooks = settings["hooks"] as? [String: Any] ?? [:]

        let commands = Self.hookEvents.map {
            (settingsKey: $0.settingsKey, command: Self.hookCommand(hookScript: hookScriptPath, event: $0.event))
        }

        guard let updatedHooks = Self.hooks(installing: commands, into: hooks) else { return }

        var updatedSettings = settings
        updatedSettings["hooks"] = updatedHooks
        try Self.writeSettings(updatedSettings)
    }

    func isHookInstalled() -> Bool {
        Self.fileContainsMuxyMarker(at: Self.settingsPath)
    }

    static func fileContainsMuxyMarker(at path: String) -> Bool {
        guard let contents = try? String(contentsOfFile: path, encoding: .utf8) else { return false }
        return contents.contains(muxyMarker)
    }

    func uninstall() throws {
        guard FileManager.default.fileExists(atPath: Self.settingsPath) else { return }
        guard isHookInstalled() else { return }
        var settings = try Self.readSettings()
        guard let hooks = settings["hooks"] as? [String: Any] else { return }

        settings["hooks"] = Self.hooks(uninstallingFrom: hooks)
        try Self.writeSettings(settings)
    }

    static func hooks(
        installing commands: [(settingsKey: String, command: String)],
        into hooks: [String: Any]
    ) -> [String: Any]? {
        let alreadyInstalled = commands.allSatisfy {
            muxyHookMatches(entries: hooks[$0.settingsKey] as? [[String: Any]], expectedCommand: $0.command)
        }
        guard !alreadyInstalled else { return nil }

        var updatedHooks = hooks
        for entry in commands {
            updatedHooks[entry.settingsKey] = mergeHookArray(
                existing: hooks[entry.settingsKey] as? [[String: Any]],
                muxyHook: buildHookEntry(command: entry.command)
            )
        }
        return updatedHooks
    }

    static func hooks(uninstallingFrom hooks: [String: Any]) -> [String: Any] {
        var result = hooks
        for key in hookEvents.map(\.settingsKey) {
            guard var entries = result[key] as? [[String: Any]] else { continue }
            entries.removeAll { isMuxyHookEntry($0) }
            if entries.isEmpty {
                result.removeValue(forKey: key)
            } else {
                result[key] = entries
            }
        }
        return result
    }

    static func hookCommand(hookScript: String, event: String) -> String {
        "'\(hookScript)' \(event) # \(muxyMarker)"
    }

    private static func buildHookEntry(command: String) -> [String: Any] {
        [
            "matcher": "",
            "hooks": [
                [
                    "type": "command",
                    "command": command,
                    "timeout": 10,
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
        guard FileManager.default.fileExists(atPath: settingsPath) else { return [:] }
        let data = try Data(contentsOf: URL(fileURLWithPath: settingsPath))
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { return [:] }
        return json
    }

    private static func writeSettings(_ settings: [String: Any]) throws {
        let dirPath = (settingsPath as NSString).deletingLastPathComponent
        try FileManager.default.createDirectory(atPath: dirPath, withIntermediateDirectories: true)

        let fileURL = URL(fileURLWithPath: settingsPath)
        if FileManager.default.fileExists(atPath: settingsPath) {
            let backupPath = settingsPath + ".muxy-backup"
            let backupURL = URL(fileURLWithPath: backupPath)
            try? FileManager.default.removeItem(at: backupURL)
            try FileManager.default.copyItem(at: fileURL, to: backupURL)
        }

        let data = try JSONSerialization.data(withJSONObject: settings, options: [.prettyPrinted, .sortedKeys])
        try data.write(to: fileURL, options: .atomic)
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.privateFile],
            ofItemAtPath: settingsPath
        )
    }
}
