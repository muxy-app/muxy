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
        agentCLIExecutablePath() != nil
    }

    func agentCLIExecutablePath() -> String? {
        ProviderExecutableLocator.executablePath(
            names: [agentLaunchConfiguration.executable],
            homeDirectory: NSHomeDirectory(),
            pathEnvironment: LoginShellPath.current,
            includeSystemWide: true
        )
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

    var configPaths: [String] { [Self.settingsPath] }

    func verify(hookScriptPath: String) -> HookVerification {
        let expected = Self.hookEvents.map {
            Self.hookCommand(hookScript: hookScriptPath, event: $0.event)
        }
        return Self.verifyNestedHooks(
            at: Self.settingsPath,
            keys: Self.hookEvents.map(\.settingsKey),
            expectedCommands: expected
        )
    }

    static func verifyNestedHooks(
        at path: String,
        keys: [String],
        expectedCommands: [String]
    ) -> HookVerification {
        guard fileContainsMuxyMarker(at: path) else { return .needsRepair }
        guard let settings = try? readJSON(at: path),
              let hooks = settings["hooks"] as? [String: Any]
        else { return .needsRepair }

        for (key, expected) in zip(keys, expectedCommands) {
            let entries = hooks[key] as? [[String: Any]]
            guard hasSingleMuxyHook(entries: entries, expectedCommand: expected) else {
                return .needsRepair
            }
        }
        return .satisfied
    }

    static func readJSON(at path: String) throws -> [String: Any] {
        let data = try Data(contentsOf: URL(fileURLWithPath: path))
        guard !data.isEmpty, let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { return [:] }
        return json
    }

    static func commandStrings(inNested entries: [[String: Any]]?) -> [String] {
        guard let entries else { return [] }
        return entries.flatMap { entry -> [String] in
            guard let hooks = entry["hooks"] as? [[String: Any]] else { return [] }
            return hooks.compactMap { $0["command"] as? String }
        }
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
            hasSingleMuxyHook(
                entries: hooks[$0.settingsKey] as? [[String: Any]],
                expectedCommand: $0.command
            )
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
            guard let existing = result[key] as? [[String: Any]] else { continue }
            let entries = removingMuxyHooks(fromNested: existing)
            if entries.isEmpty {
                result.removeValue(forKey: key)
            } else {
                result[key] = entries
            }
        }
        return result
    }

    static func hookCommand(hookScript: String, event: String) -> String {
        "\(ShellEscaper.quote(hookScript)) \(event) # \(muxyMarker)"
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

    static func hasSingleMuxyHook(entries: [[String: Any]]?, expectedCommand: String) -> Bool {
        guard let entries else { return false }
        let muxyCommands = commandStrings(inNested: entries).filter { isMuxyCommand($0) }
        return muxyCommands == [expectedCommand]
    }

    private static func mergeHookArray(
        existing: [[String: Any]]?,
        muxyHook: [String: Any]
    ) -> [[String: Any]] {
        var entries = removingMuxyHooks(fromNested: existing ?? [])
        entries.append(muxyHook)
        return entries
    }

    static func removingMuxyHooks(fromNested entries: [[String: Any]]) -> [[String: Any]] {
        entries.compactMap { entry in
            guard let hooks = entry["hooks"] as? [[String: Any]] else { return entry }
            let foreignHooks = hooks.filter { hook in
                guard let command = hook["command"] as? String else { return true }
                return !isMuxyCommand(command)
            }
            guard !foreignHooks.isEmpty else { return nil }
            guard foreignHooks.count != hooks.count else { return entry }
            var updatedEntry = entry
            updatedEntry["hooks"] = foreignHooks
            return updatedEntry
        }
    }

    private static func isMuxyCommand(_ command: String) -> Bool {
        command.contains(muxyMarker)
    }

    private static func readSettings() throws -> [String: Any] {
        guard FileManager.default.fileExists(atPath: settingsPath) else { return [:] }
        let data = try Data(contentsOf: URL(fileURLWithPath: settingsPath))
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { return [:] }
        return json
    }

    private static func writeSettings(_ settings: [String: Any]) throws {
        try HookConfigWriter.write(settings, to: settingsPath)
    }
}
