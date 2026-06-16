import Foundation

struct CodexProvider: AIProviderIntegration {
    let id = "codex"
    let displayName = "Codex"
    let socketTypeKey = "codex_hook"
    let iconName = "codex"
    let executableNames = ["codex"]
    let hookScriptName = "muxy-codex-hook"

    private static let muxyMarker = "muxy-notification-hook"
    private static let installedEvents = ["Stop"]
    private static let removableEvents = installedEvents + ["Notification"]
    private let homeDirectory: String
    private let pathEnvironment: () -> String
    private let hooksPath: String

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
        CodexExecutableLocator.isInstalled(
            names: executableNames,
            homeDirectory: homeDirectory,
            pathEnvironment: pathEnvironment()
        )
    }

    func install(hookScriptPath: String) throws {
        let settings = try readSettings()
        let hooks = settings["hooks"] as? [String: Any] ?? [:]
        var updatedSettings = settings
        var updatedHooks = hooks
        var changed = false

        for event in Self.removableEvents where !Self.installedEvents.contains(event) {
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
            let command = Self.hookCommand(hookScript: hookScriptPath, event: event.lowercased())
            let entry = Self.buildHookEntry(command: command)
            let existing = updatedHooks[event] as? [[String: Any]]
            guard !Self.muxyHookMatches(entries: existing, expectedCommand: command) || Self.muxyHookEntryCount(existing) != 1
            else { continue }
            updatedHooks[event] = Self.mergeHookArray(existing: existing, muxyHook: entry)
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

private enum CodexExecutableLocator {
    static func isInstalled(
        names: [String],
        homeDirectory: String,
        pathEnvironment: String
    ) -> Bool {
        let directories = candidateDirectories(
            homeDirectory: homeDirectory,
            pathEnvironment: pathEnvironment
        )
        return names.contains { name in
            directories.contains { directory in
                let path = URL(fileURLWithPath: directory).appendingPathComponent(name).path
                return FileManager.default.isExecutableFile(atPath: path)
            }
        }
    }

    private static func candidateDirectories(
        homeDirectory: String,
        pathEnvironment: String
    ) -> [String] {
        let pathDirectories = pathEnvironment
            .split(separator: ":", omittingEmptySubsequences: true)
            .map(String.init)
        let directories = [
            "\(homeDirectory)/.local/bin",
            "\(homeDirectory)/.npm-global/bin",
            "/usr/local/bin",
            "/opt/homebrew/bin",
        ] + pathDirectories
        var seen = Set<String>()
        return directories.filter { seen.insert($0).inserted }
    }
}
