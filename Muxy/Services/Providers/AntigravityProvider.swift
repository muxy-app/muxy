import Foundation

struct AntigravityProvider: AIProviderIntegration, AIAgentLaunchProvider {
    let id = "antigravity"
    let displayName = "Antigravity CLI"
    let socketTypeKey = "antigravity_hook"
    let iconName = "antigravity"
    let executableNames = ["agy", "antigravity"]
    let hookScriptName = "muxy-antigravity-hook"

    var agentLaunchConfiguration: AIAgentLaunchConfiguration {
        AIAgentLaunchConfiguration(
            executable: "agy",
            headlessArguments: [
                "--print",
                "--output-format",
                "text",
                "--mode=plan",
            ]
        )
    }

    private static let muxyMarker = "muxy-notification-hook"
    private static let hookName = "muxy-notify"

    enum HookStructure: Equatable {
        case grouped
        case flat
    }

    struct HookEvent: Equatable {
        let settingsKey: String
        let event: String
        let structure: HookStructure
    }

    static let hookEvents: [HookEvent] = [
        HookEvent(settingsKey: "PreInvocation", event: "PreInvocation", structure: .flat),
        HookEvent(settingsKey: "PreToolUse", event: "PreToolUse", structure: .grouped),
        HookEvent(settingsKey: "PostToolUse", event: "PostToolUse", structure: .grouped),
        HookEvent(settingsKey: "Stop", event: "Stop", structure: .flat),
    ]

    private let homeDirectory: String
    private let pathEnvironment: @Sendable () -> String

    init(
        homeDirectory: String = NSHomeDirectory(),
        pathEnvironment: @escaping @Sendable () -> String = { LoginShellPath.current }
    ) {
        self.homeDirectory = homeDirectory
        self.pathEnvironment = pathEnvironment
    }

    init(
        homeDirectory: String = NSHomeDirectory(),
        pathEnvironment: String
    ) {
        self.init(homeDirectory: homeDirectory, pathEnvironment: { pathEnvironment })
    }

    private var configDir: String { homeDirectory + "/.gemini/config" }
    private var hooksPath: String { configDir + "/hooks.json" }

    func isToolInstalled() -> Bool {
        agentCLIExecutablePath() != nil
    }

    func agentCLIExecutablePath() -> String? {
        ProviderExecutableLocator.executablePath(
            names: ["agy", "antigravity"],
            homeDirectory: homeDirectory,
            pathEnvironment: pathEnvironment(),
            includeSystemWide: homeDirectory == NSHomeDirectory(),
            homeRelativeBins: [".local/bin", ".gemini/antigravity-cli/bin"]
        )
    }

    func isHookInstalled() -> Bool {
        ClaudeCodeProvider.fileContainsMuxyMarker(at: hooksPath)
    }

    var configPaths: [String] { [hooksPath] }

    func verify(hookScriptPath: String) -> HookVerification {
        guard ClaudeCodeProvider.fileContainsMuxyMarker(at: hooksPath) else { return .needsRepair }
        guard let settings = try? ClaudeCodeProvider.readJSON(at: hooksPath),
              let muxyHookConfig = settings[Self.hookName] as? [String: Any]
        else { return .needsRepair }

        for event in Self.hookEvents {
            let expected = Self.hookCommand(hookScript: hookScriptPath, event: event.event)
            let entries = muxyHookConfig[event.settingsKey] as? [[String: Any]]
            let isSatisfied: Bool = switch event.structure {
            case .grouped:
                Self.hasSingleMuxyGroupedHook(entries: entries, expectedCommand: expected)
            case .flat:
                Self.hasSingleMuxyFlatHook(entries: entries, expectedCommand: expected)
            }
            guard isSatisfied else { return .needsRepair }
        }
        return .satisfied
    }

    struct HookInstallationCommand: Equatable {
        let settingsKey: String
        let command: String
        let structure: HookStructure
    }

    func install(hookScriptPath: String) throws {
        let existing = try Self.readHooksFile(at: hooksPath)
        try Self.validateManagedHooks(in: existing)
        let commands = Self.hookEvents.map {
            HookInstallationCommand(
                settingsKey: $0.settingsKey,
                command: Self.hookCommand(hookScript: hookScriptPath, event: $0.event),
                structure: $0.structure
            )
        }

        guard let updated = Self.hooks(installing: commands, into: existing) else { return }
        try Self.writeHooksFile(updated, at: hooksPath)
    }

    private static func validateManagedHooks(in hooks: [String: Any]) throws {
        guard let managedValue = hooks[hookName] else { return }
        guard let managedHooks = managedValue as? [String: Any] else {
            throw CocoaError(.fileReadCorruptFile)
        }
        for event in hookEvents {
            guard let eventValue = managedHooks[event.settingsKey] else { continue }
            guard eventValue is [[String: Any]] else {
                throw CocoaError(.fileReadCorruptFile)
            }
        }
    }

    func uninstall() throws {
        guard FileManager.default.fileExists(atPath: hooksPath) else { return }
        guard isHookInstalled() else { return }
        let settings = try Self.readHooksFile(at: hooksPath)

        let cleaned = Self.hooks(uninstallingFrom: settings)
        if cleaned.isEmpty {
            try FileManager.default.removeItem(atPath: hooksPath)
        } else {
            try Self.writeHooksFile(cleaned, at: hooksPath)
        }
    }

    static func hooks(
        installing commands: [HookInstallationCommand],
        into existingConfig: [String: Any]
    ) -> [String: Any]? {
        var muxyHookConfig = existingConfig[hookName] as? [String: Any] ?? [:]

        let alreadyInstalled = commands.allSatisfy { item in
            let entries = muxyHookConfig[item.settingsKey] as? [[String: Any]]
            switch item.structure {
            case .grouped:
                return hasSingleMuxyGroupedHook(entries: entries, expectedCommand: item.command)
            case .flat:
                return hasSingleMuxyFlatHook(entries: entries, expectedCommand: item.command)
            }
        }
        guard !alreadyInstalled else { return nil }

        for item in commands {
            let existing = muxyHookConfig[item.settingsKey] as? [[String: Any]]
            switch item.structure {
            case .grouped:
                muxyHookConfig[item.settingsKey] = mergeGroupedHookArray(existing: existing, command: item.command)
            case .flat:
                muxyHookConfig[item.settingsKey] = mergeFlatHookArray(existing: existing, command: item.command)
            }
        }

        var updated = existingConfig
        updated[hookName] = muxyHookConfig
        return updated
    }

    static func hooks(uninstallingFrom hooks: [String: Any]) -> [String: Any] {
        var result = hooks
        if var muxyHookConfig = result[hookName] as? [String: Any] {
            for event in hookEvents {
                guard let existing = muxyHookConfig[event.settingsKey] as? [[String: Any]] else { continue }
                let entries: [[String: Any]] = switch event.structure {
                case .grouped:
                    ClaudeCodeProvider.removingMuxyHooks(fromNested: existing)
                case .flat:
                    removingMuxyFlatHooks(from: existing)
                }
                if entries.isEmpty {
                    muxyHookConfig.removeValue(forKey: event.settingsKey)
                } else {
                    muxyHookConfig[event.settingsKey] = entries
                }
            }
            if muxyHookConfig.isEmpty {
                result.removeValue(forKey: hookName)
            } else {
                result[hookName] = muxyHookConfig
            }
        }
        return result
    }

    static func hookCommand(hookScript: String, event: String) -> String {
        "\(ShellEscaper.quote(hookScript)) \(event) # \(muxyMarker)"
    }

    static func hasSingleMuxyGroupedHook(entries: [[String: Any]]?, expectedCommand: String) -> Bool {
        ClaudeCodeProvider.hasSingleMuxyHook(entries: entries, expectedCommand: expectedCommand)
    }

    static func hasSingleMuxyFlatHook(entries: [[String: Any]]?, expectedCommand: String) -> Bool {
        guard let entries else { return false }
        let muxyCommands = entries.compactMap { $0["command"] as? String }.filter { isMuxyCommand($0) }
        return muxyCommands == [expectedCommand]
    }

    private static func buildGroupedHookEntry(command: String) -> [String: Any] {
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

    private static func buildFlatHookEntry(command: String) -> [String: Any] {
        [
            "type": "command",
            "command": command,
            "timeout": 10,
        ]
    }

    private static func mergeGroupedHookArray(
        existing: [[String: Any]]?,
        command: String
    ) -> [[String: Any]] {
        var entries = ClaudeCodeProvider.removingMuxyHooks(fromNested: existing ?? [])
        entries.append(buildGroupedHookEntry(command: command))
        return entries
    }

    private static func mergeFlatHookArray(
        existing: [[String: Any]]?,
        command: String
    ) -> [[String: Any]] {
        var entries = removingMuxyFlatHooks(from: existing ?? [])
        entries.append(buildFlatHookEntry(command: command))
        return entries
    }

    static func removingMuxyFlatHooks(from entries: [[String: Any]]) -> [[String: Any]] {
        entries.filter { entry in
            guard let command = entry["command"] as? String else { return true }
            return !isMuxyCommand(command)
        }
    }

    private static func isMuxyCommand(_ command: String) -> Bool {
        command.contains(muxyMarker)
    }

    private static func readHooksFile(at path: String) throws -> [String: Any] {
        guard FileManager.default.fileExists(atPath: path) else { return [:] }
        let data = try Data(contentsOf: URL(fileURLWithPath: path))
        guard !data.isEmpty else { return [:] }
        let json = try JSONSerialization.jsonObject(with: data)
        guard let object = json as? [String: Any] else {
            throw CocoaError(.fileReadCorruptFile)
        }
        return object
    }

    private static func writeHooksFile(_ settings: [String: Any], at path: String) throws {
        try HookConfigWriter.write(settings, to: path)
    }
}
