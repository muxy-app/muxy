import Foundation

struct KiroProvider: AIProviderIntegration, AIAgentLaunchProvider {
    let id = "kiro"
    let displayName = "Kiro CLI"
    let socketTypeKey = "kiro_hook"
    let iconName = "kiro"
    let executableNames = ["kiro-cli"]
    let hookScriptName = "muxy-kiro-hook"

    var agentLaunchConfiguration: AIAgentLaunchConfiguration {
        AIAgentLaunchConfiguration(
            executable: "kiro-cli",
            headlessArguments: ["chat", "--no-interactive"]
        )
    }

    private static let muxyMarker = "muxy-notification-hook"
    private static let triggers = ["UserPromptSubmit", "PreToolUse", "Stop"]
    private let homeDirectory: String

    init(homeDirectory: String = NSHomeDirectory()) {
        self.homeDirectory = homeDirectory
    }

    private var hooksPath: String {
        homeDirectory + "/.kiro/hooks/muxy-notify.json"
    }

    var configPaths: [String] { [hooksPath] }

    func isHookInstalled() -> Bool {
        ClaudeCodeProvider.fileContainsMuxyMarker(at: hooksPath)
    }

    func agentCLIExecutablePath() -> String? {
        ProviderExecutableLocator.executablePath(
            names: ["kiro-cli"],
            homeDirectory: homeDirectory,
            pathEnvironment: LoginShellPath.current,
            includeSystemWide: homeDirectory == NSHomeDirectory(),
            homeRelativeBins: [".local/bin"]
        )
    }

    func install(hookScriptPath: String) throws {
        let settings = try readSettings()
        let existingHooks = settings["hooks"] as? [[String: Any]] ?? []
        guard settings["version"] as? String != "v1" || !Self.isInstalled(hooks: existingHooks, hookScriptPath: hookScriptPath)
        else { return }

        let newHooks = Self.hooks(installingAt: hookScriptPath, into: existingHooks)
        var updatedSettings = settings
        updatedSettings["version"] = "v1"
        updatedSettings["hooks"] = newHooks
        try writeSettings(updatedSettings)
    }

    func uninstall() throws {
        guard FileManager.default.fileExists(atPath: hooksPath) else { return }
        guard isHookInstalled() else { return }

        let settings = try readSettings()
        let hooks = settings["hooks"] as? [[String: Any]] ?? []
        let cleaned = Self.hooks(uninstallingFrom: hooks)

        if cleaned.isEmpty {
            try FileManager.default.removeItem(atPath: hooksPath)
        } else {
            var updatedSettings = settings
            updatedSettings["hooks"] = cleaned
            try writeSettings(updatedSettings)
        }
    }

    func verify(hookScriptPath: String) -> HookVerification {
        guard isHookInstalled() else { return .needsRepair }
        guard let settings = try? readSettings(),
              settings["version"] as? String == "v1"
        else { return .needsRepair }

        let hooks = settings["hooks"] as? [[String: Any]] ?? []
        let muxyEntries = hooks.filter { Self.isMuxyEntry($0) }
        guard muxyEntries.count == Self.triggers.count else { return .needsRepair }

        let expected = Self.expectedCommands(hookScriptPath: hookScriptPath)
        for (trigger, command) in expected {
            let entries = muxyEntries.filter { $0["trigger"] as? String == trigger }
            guard entries.count == 1,
                  let action = entries.first?["action"] as? [String: Any],
                  action["command"] as? String == command
            else { return .needsRepair }
        }
        return .satisfied
    }

    static func hooks(installingAt hookScriptPath: String, into hooks: [[String: Any]]) -> [[String: Any]] {
        let foreign = hooks.filter { !isMuxyEntry($0) }
        let new = Self.triggers.map { hookEntry(hookScriptPath: hookScriptPath, trigger: $0) }
        return foreign + new
    }

    static func hooks(uninstallingFrom hooks: [[String: Any]]) -> [[String: Any]] {
        hooks.filter { !isMuxyEntry($0) }
    }

    static func hookCommand(hookScript: String, trigger: String) -> String {
        "\(ShellEscaper.quote(hookScript)) \(trigger) # \(muxyMarker)"
    }

    private static func hookEntry(hookScriptPath: String, trigger: String) -> [String: Any] {
        [
            "name": "muxy-notify-\(trigger)",
            "trigger": trigger,
            "action": [
                "type": "command",
                "command": hookCommand(hookScript: hookScriptPath, trigger: trigger),
            ] as [String: Any],
            "timeout": 10,
        ]
    }

    private static func isMuxyEntry(_ entry: [String: Any]) -> Bool {
        guard let action = entry["action"] as? [String: Any],
              let command = action["command"] as? String
        else { return false }
        return command.contains(muxyMarker)
    }

    private static func expectedCommands(hookScriptPath: String) -> [(String, String)] {
        Self.triggers.map { ($0, hookCommand(hookScript: hookScriptPath, trigger: $0)) }
    }

    private static func isInstalled(hooks: [[String: Any]], hookScriptPath: String) -> Bool {
        let muxyEntries = hooks.filter { isMuxyEntry($0) }
        guard muxyEntries.count == triggers.count else { return false }
        for trigger in triggers {
            let entries = muxyEntries.filter { $0["trigger"] as? String == trigger }
            guard entries.count == 1,
                  let action = entries.first?["action"] as? [String: Any],
                  action["command"] as? String == hookCommand(hookScript: hookScriptPath, trigger: trigger)
            else { return false }
        }
        return true
    }

    private func readSettings() throws -> [String: Any] {
        guard FileManager.default.fileExists(atPath: hooksPath) else { return [:] }
        let data = try Data(contentsOf: URL(fileURLWithPath: hooksPath))
        guard !data.isEmpty,
              let json = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return [:] }
        return json
    }

    private func writeSettings(_ settings: [String: Any]) throws {
        try HookConfigWriter.write(settings, to: hooksPath)
    }
}
