import Foundation

struct CommandCodeProvider: AIProviderIntegration, AIAgentLaunchProvider {
    let id = "cmd"
    let displayName = "Command Code"
    let socketTypeKey = "cmd_cli"
    let iconName = "cmd"
    let executableNames = ["cmd"]
    let hookScriptName = "muxy-cmd-hook"

    var agentLaunchConfiguration: AIAgentLaunchConfiguration {
        AIAgentLaunchConfiguration(
            executable: "cmd",
            headlessArguments: [
                "-p",
                "--permission-mode",
                "bypass",
            ]
        )
    }

    private static let muxyMarker = "muxy-notification-hook"
    private static let hookTimeoutSeconds = 10
    private static let settingsPath = NSHomeDirectory() + "/.commandcode/settings.json"
    private static let installedEvents: [(settingsKey: String, event: String)] = [
        ("PreToolUse", "pre-tool-use"),
        ("Stop", "stop"),
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
            homeRelativeBins: [".local/bin", ".npm-global/bin"]
        )
    }

    func isHookInstalled() -> Bool {
        ClaudeCodeProvider.fileContainsMuxyMarker(at: Self.settingsPath)
    }

    var configPaths: [String] { [Self.settingsPath] }

    func verify(hookScriptPath: String) -> HookVerification {
        guard ClaudeCodeProvider.fileContainsMuxyMarker(at: Self.settingsPath) else { return .needsRepair }
        guard let settings = try? ClaudeCodeProvider.readJSON(at: Self.settingsPath),
              let hooks = settings["hooks"] as? [String: Any]
        else { return .needsRepair }

        for event in Self.installedEvents {
            let expected = Self.hookCommand(hookScript: hookScriptPath, event: event.event)
            let entries = hooks[event.settingsKey] as? [[String: Any]]
            guard Self.muxyHookMatches(entries: entries, expectedCommand: expected),
                  Self.muxyHookEntryCount(entries) == 1
            else { return .needsRepair }
        }
        return .satisfied
    }

    func install(hookScriptPath: String) throws {
        let settings = try Self.readSettings()
        let hooks = settings["hooks"] as? [String: Any] ?? [:]
        var updatedSettings = settings
        var updatedHooks = hooks
        var changed = false

        for event in Self.installedEvents {
            let command = Self.hookCommand(hookScript: hookScriptPath, event: event.event)
            let entry = Self.buildHookEntry(command: command)
            let existing = updatedHooks[event.settingsKey] as? [[String: Any]]
            guard !Self.muxyHookMatches(entries: existing, expectedCommand: command)
                || Self.muxyHookEntryCount(existing) != 1
            else { continue }
            updatedHooks[event.settingsKey] = Self.mergeHookArray(existing: existing, muxyHook: entry)
            changed = true
        }

        guard changed else { return }
        updatedSettings["hooks"] = updatedHooks
        try Self.writeSettings(updatedSettings)
    }

    func uninstall() throws {
        guard FileManager.default.fileExists(atPath: Self.settingsPath) else { return }
        guard isHookInstalled() else { return }
        var settings = try Self.readSettings()
        guard var hooks = settings["hooks"] as? [String: Any] else { return }

        for event in Self.installedEvents {
            guard let entries = hooks[event.settingsKey] as? [[String: Any]] else { continue }
            let result = Self.removingMuxyHooks(from: entries)
            if result.entries.isEmpty {
                hooks.removeValue(forKey: event.settingsKey)
            } else {
                hooks[event.settingsKey] = result.entries
            }
        }

        settings["hooks"] = hooks
        try Self.writeSettings(settings)
    }

    private static func hookCommand(hookScript: String, event: String) -> String {
        "\(ShellEscaper.quote(hookScript)) \(event) # \(muxyMarker)"
    }

    private static func buildHookEntry(command: String) -> [String: Any] {
        [
            "hooks": [
                [
                    "type": "command",
                    "command": command,
                    "timeout": hookTimeoutSeconds,
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
        entries = removingMuxyHooks(from: entries).entries
        entries.append(muxyHook)
        return entries
    }

    private static func removingMuxyHooks(from entries: [[String: Any]]) -> (entries: [[String: Any]], changed: Bool) {
        var changed = false
        let filteredEntries = entries.compactMap { entry -> [String: Any]? in
            guard var hooks = entry["hooks"] as? [[String: Any]] else { return entry }
            let originalHookCount = hooks.count
            hooks.removeAll { isMuxyHook($0) }
            guard hooks.count != originalHookCount else { return entry }
            changed = true
            guard !hooks.isEmpty else { return nil }
            var updatedEntry = entry
            updatedEntry["hooks"] = hooks
            return updatedEntry
        }
        return (filteredEntries, changed)
    }

    private static func isMuxyHook(_ hook: [String: Any]) -> Bool {
        guard let command = hook["command"] as? String else { return false }
        return command.contains(muxyMarker)
    }

    private static func muxyHookEntryCount(_ entries: [[String: Any]]?) -> Int {
        entries?.reduce(0) { count, entry in
            guard let hooks = entry["hooks"] as? [[String: Any]] else { return count }
            return count + hooks.count(where: { isMuxyHook($0) })
        } ?? 0
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
