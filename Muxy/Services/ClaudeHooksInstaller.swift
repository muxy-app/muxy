import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "ClaudeHooksInstaller")

enum ClaudeHooksInstaller {
    private static let settingsPath = NSHomeDirectory() + "/.claude/settings.json"
    private static let muxyMarker = "muxy-notification-hook"

    static func installIfNeeded() {
        #if DEBUG
        guard ProcessInfo.processInfo.environment["FF_CLAUDE_HOOKS"] != nil else {
            logger.info("Skipping Claude hooks install in dev mode (set FF_CLAUDE_HOOKS=true to enable)")
            return
        }
        #endif
        guard isClaudeInstalled() else { return }
        guard let hookScript = MuxyNotificationHooks.hookScriptPath else {
            logger.info("Hook script not found, skipping Claude hooks install")
            return
        }

        do {
            try installHooks(hookScript: hookScript)
            logger.info("Claude Code hooks installed")
        } catch {
            logger.error("Failed to install Claude hooks: \(error.localizedDescription)")
        }
    }

    static func uninstall() {
        #if DEBUG
        guard ProcessInfo.processInfo.environment["FF_CLAUDE_HOOKS"] != nil else { return }
        #endif
        guard FileManager.default.fileExists(atPath: settingsPath) else { return }
        do {
            try removeHooks()
            logger.info("Claude Code hooks removed")
        } catch {
            logger.error("Failed to remove Claude hooks: \(error.localizedDescription)")
        }
    }

    private static func isClaudeInstalled() -> Bool {
        let knownPaths = [
            NSHomeDirectory() + "/.local/bin/claude",
            "/usr/local/bin/claude",
            "/opt/homebrew/bin/claude",
        ]
        return knownPaths.contains { FileManager.default.isExecutableFile(atPath: $0) }
    }

    private static func installHooks(hookScript: String) throws {
        var settings = try readSettings()
        var hooks = settings["hooks"] as? [String: Any] ?? [:]

        let stopHook = buildHookEntry(hookScript: hookScript, event: "stop")
        let notificationHook = buildHookEntry(hookScript: hookScript, event: "notification")

        hooks["Stop"] = mergeHookArray(existing: hooks["Stop"] as? [[String: Any]], muxyHook: stopHook)
        hooks["Notification"] = mergeHookArray(
            existing: hooks["Notification"] as? [[String: Any]],
            muxyHook: notificationHook
        )

        settings["hooks"] = hooks
        try writeSettings(settings)
    }

    private static func removeHooks() throws {
        var settings = try readSettings()
        guard var hooks = settings["hooks"] as? [String: Any] else { return }

        for key in ["Stop", "Notification"] {
            guard var entries = hooks[key] as? [[String: Any]] else { continue }
            entries.removeAll { isMuxyHookEntry($0) }
            if entries.isEmpty {
                hooks.removeValue(forKey: key)
            } else {
                hooks[key] = entries
            }
        }

        settings["hooks"] = hooks
        try writeSettings(settings)
    }

    private static func buildHookEntry(hookScript: String, event: String) -> [String: Any] {
        [
            "matcher": "",
            "hooks": [
                [
                    "type": "command",
                    "command": "'\(hookScript)' \(event) # \(muxyMarker)",
                    "timeout": 10,
                ] as [String: Any],
            ],
        ]
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
        guard FileManager.default.fileExists(atPath: settingsPath) else {
            return [:]
        }
        let data = try Data(contentsOf: URL(fileURLWithPath: settingsPath))
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return [:]
        }
        return json
    }

    private static func writeSettings(_ settings: [String: Any]) throws {
        let dirPath = (settingsPath as NSString).deletingLastPathComponent
        try FileManager.default.createDirectory(atPath: dirPath, withIntermediateDirectories: true)
        let data = try JSONSerialization.data(withJSONObject: settings, options: [.prettyPrinted, .sortedKeys])
        try data.write(to: URL(fileURLWithPath: settingsPath), options: .atomic)
    }
}
