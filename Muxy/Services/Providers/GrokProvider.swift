import Foundation

struct GrokProvider: AIProviderIntegration {
    let id = "grok"
    let displayName = "Grok"
    let socketTypeKey = "grok_hook"
    let iconName = "grok"
    let executableNames = ["grok"]
    let hookScriptName = "muxy-grok-hook"

    private static let muxyMarker = "muxy-notification-hook"
    private static let hookFileName = "muxy-notify.json"

    static let hookEvents: [(settingsKey: String, event: String)] = [
        ("Stop", "stop"),
        ("Notification", "notification"),
        ("UserPromptSubmit", "user-prompt-submit"),
        ("PreToolUse", "pre-tool-use"),
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

    private var hooksDir: String { homeDirectory + "/.grok/hooks" }
    private var hookFilePath: String { hooksDir + "/" + Self.hookFileName }

    func isToolInstalled() -> Bool {
        ProviderExecutableLocator.isInstalled(
            names: executableNames,
            homeDirectory: homeDirectory,
            pathEnvironment: pathEnvironment(),
            includeSystemWide: homeDirectory == NSHomeDirectory()
        )
    }

    func isHookInstalled() -> Bool {
        ClaudeCodeProvider.fileContainsMuxyMarker(at: hookFilePath)
    }

    func install(hookScriptPath: String) throws {
        let existing = try Self.readHooksFile(at: hookFilePath)
        let hooks = existing["hooks"] as? [String: Any] ?? [:]

        let commands = Self.hookEvents.map {
            (settingsKey: $0.settingsKey, command: Self.hookCommand(hookScript: hookScriptPath, event: $0.event))
        }

        guard let updatedHooks = Self.hooks(installing: commands, into: hooks) else { return }

        var updated = existing
        updated["hooks"] = updatedHooks
        try Self.writeHooksFile(updated, at: hookFilePath, hooksDir: hooksDir)
    }

    func uninstall() throws {
        guard FileManager.default.fileExists(atPath: hookFilePath) else { return }
        var settings = try Self.readHooksFile(at: hookFilePath)
        guard let hooks = settings["hooks"] as? [String: Any] else { return }

        let cleaned = Self.hooks(uninstallingFrom: hooks)
        if cleaned.isEmpty {
            settings.removeValue(forKey: "hooks")
        } else {
            settings["hooks"] = cleaned
        }
        try Self.writeHooksFile(settings, at: hookFilePath, hooksDir: hooksDir)
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

    private static func readHooksFile(at path: String) throws -> [String: Any] {
        guard FileManager.default.fileExists(atPath: path) else { return [:] }
        let data = try Data(contentsOf: URL(fileURLWithPath: path))
        guard !data.isEmpty else { return [:] }
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { return [:] }
        return json
    }

    private static func writeHooksFile(_ settings: [String: Any], at path: String, hooksDir: String) throws {
        try FileManager.default.createDirectory(
            atPath: hooksDir,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: FilePermissions.privateDirectory]
        )

        let fileURL = URL(fileURLWithPath: path)
        if FileManager.default.fileExists(atPath: path) {
            let backupPath = path + ".muxy-backup"
            let backupURL = URL(fileURLWithPath: backupPath)
            try? FileManager.default.removeItem(at: backupURL)
            try FileManager.default.copyItem(at: fileURL, to: backupURL)
        }

        let data = try JSONSerialization.data(withJSONObject: settings, options: [.prettyPrinted, .sortedKeys])
        try data.write(to: fileURL, options: .atomic)
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.privateFile],
            ofItemAtPath: path
        )
    }
}
