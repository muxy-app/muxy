import Foundation

struct CopilotProvider: AIProviderIntegration, AIAgentLaunchProvider {
    let id = "copilot"
    let displayName = "GitHub Copilot"
    let socketTypeKey = "copilot_hook"
    let iconName = "copilot"
    let executableNames = ["copilot"]
    let hookScriptName = "muxy-copilot-hook"
    let requiresLoginShellEnvironmentForConfiguration = true

    var agentLaunchConfiguration: AIAgentLaunchConfiguration {
        AIAgentLaunchConfiguration(
            executable: "copilot",
            headlessArguments: [
                "--silent",
                "--no-ask-user",
                "--available-tools=",
                "-p",
            ],
            modelArgument: nil
        )
    }

    private static let muxyMarker = "muxy-notification-hook"
    private static let hookFileName = "muxy-notify.json"
    private static let hookTimeoutSeconds = 10
    private static let hookFileVersion = 1

    static let bindings: [(settingsKey: String, argument: String)] = [
        ("userPromptSubmitted", "user-prompt-submit"),
        ("preToolUse", "pre-tool-use"),
        ("notification", "notification"),
        ("agentStop", "stop"),
        ("sessionEnd", "session-end"),
        ("errorOccurred", "errorOccurred"),
    ]

    private static let obsoleteEvents = ["permissionRequest"]
    private static let managedEvents = bindings.map(\.settingsKey) + obsoleteEvents

    private let homeDirectory: String
    private let pathEnvironment: @Sendable () -> String
    private let copilotHomeEnvironment: @Sendable () -> String?
    private let copilotHomeOverride: String?

    init(
        homeDirectory: String = NSHomeDirectory(),
        pathEnvironment: @escaping @Sendable () -> String = { LoginShellPath.current },
        copilotHomeEnvironment: @escaping @Sendable () -> String? = { LoginShellPath.currentCopilotHome },
        copilotHome: String? = nil
    ) {
        self.homeDirectory = homeDirectory
        self.pathEnvironment = pathEnvironment
        self.copilotHomeEnvironment = copilotHomeEnvironment
        self.copilotHomeOverride = copilotHome
    }

    init(
        homeDirectory: String = NSHomeDirectory(),
        pathEnvironment: String,
        copilotHomeEnvironment: @escaping @Sendable () -> String? = { LoginShellPath.currentCopilotHome },
        copilotHome: String? = nil
    ) {
        self.init(
            homeDirectory: homeDirectory,
            pathEnvironment: { pathEnvironment },
            copilotHomeEnvironment: copilotHomeEnvironment,
            copilotHome: copilotHome
        )
    }

    private var copilotHome: String {
        if let copilotHomeOverride, !copilotHomeOverride.isEmpty {
            return copilotHomeOverride
        }
        if let envHome = copilotHomeEnvironment(), !envHome.isEmpty {
            return envHome
        }
        return homeDirectory + "/.copilot"
    }

    private var hooksDir: String { copilotHome + "/hooks" }
    private var hookFilePath: String { hooksDir + "/" + Self.hookFileName }

    func isToolInstalled() -> Bool {
        agentCLIExecutablePath() != nil
    }

    func agentCLIExecutablePath() -> String? {
        ProviderExecutableLocator.executablePath(
            names: [agentLaunchConfiguration.executable],
            homeDirectory: homeDirectory,
            pathEnvironment: pathEnvironment(),
            includeSystemWide: homeDirectory == NSHomeDirectory(),
            homeRelativeBins: [".local/bin", ".npm-global/bin"]
        )
    }

    func isHookInstalled() -> Bool {
        ClaudeCodeProvider.fileContainsMuxyMarker(at: hookFilePath)
    }

    var configPaths: [String] { [hookFilePath] }

    func verify(hookScriptPath: String) -> HookVerification {
        guard ClaudeCodeProvider.fileContainsMuxyMarker(at: hookFilePath) else { return .needsRepair }
        guard let settings = try? ClaudeCodeProvider.readJSON(at: hookFilePath),
              settings["version"] as? Int == Self.hookFileVersion,
              let hooks = settings["hooks"] as? [String: Any]
        else { return .needsRepair }

        for binding in Self.bindings {
            let expected = Self.buildHookEntry(
                bash: Self.hookCommand(hookScript: hookScriptPath, argument: binding.argument)
            )
            let entries = hooks[binding.settingsKey] as? [[String: Any]]
            guard Self.hasSingleMuxyHook(entries: entries, expected: expected) else {
                return .needsRepair
            }
        }
        for event in Self.obsoleteEvents {
            let entries = hooks[event] as? [[String: Any]]
            guard Self.muxyHookCount(entries) == 0 else { return .needsRepair }
        }
        return .satisfied
    }

    func install(hookScriptPath: String) throws {
        var settings = try Self.readSettings(at: hookFilePath)
        var changed = false
        if settings["version"] as? Int != Self.hookFileVersion {
            settings["version"] = Self.hookFileVersion
            changed = true
        }
        var hooks = settings["hooks"] as? [String: Any] ?? [:]

        for event in Self.obsoleteEvents {
            guard var entries = hooks[event] as? [[String: Any]] else { continue }
            let initialCount = entries.count
            entries.removeAll { Self.isMuxyHookEntry($0) }
            guard entries.count != initialCount else { continue }
            if entries.isEmpty {
                hooks.removeValue(forKey: event)
            } else {
                hooks[event] = entries
            }
            changed = true
        }

        for binding in Self.bindings {
            let expected = Self.buildHookEntry(
                bash: Self.hookCommand(hookScript: hookScriptPath, argument: binding.argument)
            )
            let existing = hooks[binding.settingsKey] as? [[String: Any]]
            if Self.hasSingleMuxyHook(entries: existing, expected: expected) {
                continue
            }
            hooks[binding.settingsKey] = Self.mergeHookArray(
                existing: existing,
                entry: expected
            )
            changed = true
        }

        guard changed else { return }
        settings["hooks"] = hooks
        try Self.writeSettings(settings, at: hookFilePath)
    }

    func uninstall() throws {
        guard FileManager.default.fileExists(atPath: hookFilePath) else { return }
        guard isHookInstalled() else { return }
        var settings = try Self.readSettings(at: hookFilePath)
        guard var hooks = settings["hooks"] as? [String: Any] else { return }

        for event in Self.managedEvents {
            guard var entries = hooks[event] as? [[String: Any]] else { continue }
            entries.removeAll { Self.isMuxyHookEntry($0) }
            if entries.isEmpty {
                hooks.removeValue(forKey: event)
            } else {
                hooks[event] = entries
            }
        }

        settings["hooks"] = hooks
        try Self.writeSettings(settings, at: hookFilePath)
    }

    static func hookCommand(hookScript: String, argument: String) -> String {
        "\(ShellEscaper.quote(hookScript)) \(argument) # \(muxyMarker)"
    }

    static func hasSingleMuxyHook(entries: [[String: Any]]?, expected: [String: Any]) -> Bool {
        let muxyEntries = entries?.filter(isMuxyHookEntry) ?? []
        guard muxyEntries.count == 1, let entry = muxyEntries.first else { return false }
        return hookEntriesMatch(entry, expected)
    }

    private static func muxyHookCount(_ entries: [[String: Any]]?) -> Int {
        entries?.count(where: isMuxyHookEntry) ?? 0
    }

    private static func mergeHookArray(existing: [[String: Any]]?, entry: [String: Any]) -> [[String: Any]] {
        var entries = existing ?? []
        entries.removeAll { isMuxyHookEntry($0) }
        entries.append(entry)
        return entries
    }

    private static func buildHookEntry(bash: String) -> [String: Any] {
        [
            "type": "command",
            "bash": bash,
            "timeoutSec": hookTimeoutSeconds,
        ]
    }

    private static func isMuxyHookEntry(_ entry: [String: Any]) -> Bool {
        guard let bash = entry["bash"] as? String else { return false }
        return bash.contains(muxyMarker)
    }

    private static func hookEntriesMatch(_ lhs: [String: Any], _ rhs: [String: Any]) -> Bool {
        guard (lhs["bash"] as? String) == (rhs["bash"] as? String),
              (lhs["type"] as? String) == (rhs["type"] as? String)
        else { return false }
        return intValue(lhs["timeoutSec"]) == intValue(rhs["timeoutSec"])
    }

    private static func intValue(_ value: Any?) -> Int? {
        if let value = value as? Int {
            return value
        }
        if let value = value as? NSNumber {
            return value.intValue
        }
        return nil
    }

    private static func readSettings(at path: String) throws -> [String: Any] {
        guard FileManager.default.fileExists(atPath: path) else { return [:] }
        let data = try Data(contentsOf: URL(fileURLWithPath: path))
        guard !data.isEmpty else { return [:] }
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { return [:] }
        return json
    }

    private static func writeSettings(_ settings: [String: Any], at path: String) throws {
        try HookConfigWriter.write(settings, to: path)
    }
}
