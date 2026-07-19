import Foundation

struct DroidProvider: AIProviderIntegration, AIAgentLaunchProvider {
    let id = "droid"
    let displayName = "Droid"
    let socketTypeKey = "droid_hook"
    let iconName = "factory"
    let executableNames = ["droid"]
    let hookScriptName = "muxy-droid-hook"

    var agentLaunchConfiguration: AIAgentLaunchConfiguration {
        AIAgentLaunchConfiguration(
            executable: "droid",
            headlessArguments: ["exec", "--output-format", "text"]
        )
    }

    private static let settingsPath = NSHomeDirectory() + "/.factory/settings.json"
    private static let muxyMarker = "muxy-notification-hook"

    static let hookEvents: [(settingsKey: String, event: String)] = [
        ("Stop", "stop"),
        ("SessionEnd", "session-end"),
        ("Notification", "notification"),
        ("UserPromptSubmit", "user-prompt-submit"),
        ("PreToolUse", "pre-tool-use"),
    ]

    func isToolInstalled() -> Bool {
        agentCLIExecutablePath() != nil
    }

    func agentCLIExecutablePath() -> String? {
        ProviderExecutableLocator.executablePath(
            names: [agentLaunchConfiguration.executable],
            homeDirectory: NSHomeDirectory(),
            pathEnvironment: LoginShellPath.current,
            includeSystemWide: true,
            homeRelativeBins: [".factory/bin", ".local/bin"]
        )
    }

    func isHookInstalled() -> Bool {
        ClaudeCodeProvider.fileContainsMuxyMarker(at: Self.settingsPath)
    }

    var configPaths: [String] { [Self.settingsPath] }

    func verify(hookScriptPath: String) -> HookVerification {
        ClaudeCodeProvider.verifyNestedHooks(
            at: Self.settingsPath,
            keys: Self.hookEvents.map(\.settingsKey),
            expectedCommands: Self.hookEvents.map {
                Self.hookCommand(hookScript: hookScriptPath, event: $0.event)
            }
        )
    }

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

    func uninstall() throws {
        guard FileManager.default.fileExists(atPath: Self.settingsPath) else { return }
        guard isHookInstalled() else { return }
        var settings = try Self.readSettings()
        guard let hooks = settings["hooks"] as? [String: Any] else { return }

        let cleaned = Self.hooks(uninstallingFrom: hooks)
        if cleaned.isEmpty {
            settings.removeValue(forKey: "hooks")
        } else {
            settings["hooks"] = cleaned
        }
        try Self.writeSettings(settings)
    }

    static func hooks(
        installing commands: [(settingsKey: String, command: String)],
        into hooks: [String: Any]
    ) -> [String: Any]? {
        let alreadyInstalled = commands.allSatisfy {
            ClaudeCodeProvider.hasSingleMuxyHook(
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
            let entries = ClaudeCodeProvider.removingMuxyHooks(fromNested: existing)
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

    private static func mergeHookArray(
        existing: [[String: Any]]?,
        muxyHook: [String: Any]
    ) -> [[String: Any]] {
        var entries = ClaudeCodeProvider.removingMuxyHooks(fromNested: existing ?? [])
        entries.append(muxyHook)
        return entries
    }

    private static func readSettings() throws -> [String: Any] {
        guard FileManager.default.fileExists(atPath: settingsPath) else { return [:] }
        let data = try Data(contentsOf: URL(fileURLWithPath: settingsPath))
        guard !data.isEmpty else { return [:] }
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { return [:] }
        return json
    }

    private static func writeSettings(_ settings: [String: Any]) throws {
        try HookConfigWriter.write(settings, to: settingsPath)
    }
}
