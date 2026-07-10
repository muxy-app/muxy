import Foundation

struct CodexProvider: AIProviderIntegration {
    let id = "codex"
    let displayName = "Codex"
    let socketTypeKey = "codex_hook"
    let iconName = "codex"
    let executableNames = ["codex"]
    let hookScriptName = "muxy-codex-hook"

    private static let muxyMarker = "muxy-notification-hook"
    private static let hookTimeoutSeconds = 10
    private static let installedEvents: [(settingsKey: String, event: String)] = [
        ("UserPromptSubmit", "user-prompt-submit"),
        ("PreToolUse", "pre-tool-use"),
        ("PermissionRequest", "permission-request"),
        ("Stop", "stop"),
    ]
    private static let removableEvents = installedEvents.map(\.settingsKey) + ["Notification"]
    private let homeDirectory: String
    private let pathEnvironment: () -> String
    private let hooksPath: String
    private var configPath: String { "\(homeDirectory)/.codex/config.toml" }

    init(
        homeDirectory: String = NSHomeDirectory(),
        pathEnvironment: @escaping () -> String = { LoginShellPath.current },
        hooksPath: String? = nil
    ) {
        self.homeDirectory = homeDirectory
        self.pathEnvironment = pathEnvironment
        self.hooksPath = hooksPath ?? "\(homeDirectory)/.codex/hooks.json"
    }

    init(
        homeDirectory: String = NSHomeDirectory(),
        pathEnvironment: String,
        hooksPath: String? = nil
    ) {
        self.init(homeDirectory: homeDirectory, pathEnvironment: { pathEnvironment }, hooksPath: hooksPath)
    }

    func isToolInstalled() -> Bool {
        ProviderExecutableLocator.isInstalled(
            names: executableNames,
            homeDirectory: homeDirectory,
            pathEnvironment: pathEnvironment(),
            includeSystemWide: homeDirectory == NSHomeDirectory(),
            homeRelativeBins: [".local/bin", ".npm-global/bin"]
        )
    }

    func isHookInstalled() -> Bool {
        ClaudeCodeProvider.fileContainsMuxyMarker(at: hooksPath)
    }

    func install(hookScriptPath: String) throws {
        if try hasExecutableInlineHooks() {
            if isHookInstalled() {
                try uninstall()
            }
            throw CodexProviderError.inlineHooksConfigured(configPath)
        }

        let settings = try readSettings()
        let hooks = settings["hooks"] as? [String: Any] ?? [:]
        var updatedSettings = settings
        var updatedHooks = hooks
        var changed = false

        let installedKeys = Set(Self.installedEvents.map(\.settingsKey))
        for event in Self.removableEvents where !installedKeys.contains(event) {
            guard let entries = updatedHooks[event] as? [[String: Any]] else { continue }
            let result = Self.removingMuxyHooks(from: entries)
            guard result.changed else { continue }
            changed = true
            if result.entries.isEmpty {
                updatedHooks.removeValue(forKey: event)
            } else {
                updatedHooks[event] = result.entries
            }
        }

        for event in Self.installedEvents {
            let command = Self.hookCommand(hookScript: hookScriptPath, event: event.event)
            let entry = Self.buildHookEntry(command: command)
            let existing = updatedHooks[event.settingsKey] as? [[String: Any]]
            guard !Self.muxyHookMatches(entries: existing, expectedCommand: command) || Self.muxyHookEntryCount(existing) != 1
            else { continue }
            updatedHooks[event.settingsKey] = Self.mergeHookArray(existing: existing, muxyHook: entry)
            changed = true
        }

        guard changed else { return }
        updatedSettings["hooks"] = updatedHooks
        try writeSettings(updatedSettings)
    }

    func uninstall() throws {
        guard FileManager.default.fileExists(atPath: hooksPath) else { return }
        var settings = try readSettings()
        guard var hooks = settings["hooks"] as? [String: Any] else { return }

        for key in Self.removableEvents {
            guard let entries = hooks[key] as? [[String: Any]] else { continue }
            let result = Self.removingMuxyHooks(from: entries)
            if result.entries.isEmpty {
                hooks.removeValue(forKey: key)
            } else {
                hooks[key] = result.entries
            }
        }

        settings["hooks"] = hooks
        try writeSettings(settings)
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

    private func hasExecutableInlineHooks() throws -> Bool {
        guard FileManager.default.fileExists(atPath: configPath) else { return false }
        let config = try String(contentsOfFile: configPath, encoding: .utf8)
        let events = Set([
            "PreToolUse",
            "PermissionRequest",
            "PostToolUse",
            "PreCompact",
            "PostCompact",
            "SessionStart",
            "UserPromptSubmit",
            "SubagentStart",
            "SubagentStop",
            "Stop",
        ])

        return config.split(whereSeparator: \.isNewline).contains { rawLine in
            let line = rawLine.prefix { $0 != "#" }
            let header = line.filter { !$0.isWhitespace && $0 != "\"" && $0 != "'" }
            guard header.hasPrefix("[[hooks."), header.hasSuffix("]]") else { return false }
            return events.contains(String(header.dropFirst(8).dropLast(2)))
        }
    }

    private func readSettings() throws -> [String: Any] {
        guard FileManager.default.fileExists(atPath: hooksPath) else { return [:] }
        let data = try Data(contentsOf: URL(fileURLWithPath: hooksPath))
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { return [:] }
        return json
    }

    private func writeSettings(_ settings: [String: Any]) throws {
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

enum CodexProviderError: LocalizedError, Equatable {
    case inlineHooksConfigured(String)

    var errorDescription: String? {
        switch self {
        case let .inlineHooksConfigured(path):
            "Codex hooks are configured in \(path); Muxy skipped hooks.json to avoid loading both"
        }
    }
}
