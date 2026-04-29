import Foundation

struct CursorProvider: AIProviderIntegration {
    let id = "cursor"
    let displayName = "Cursor CLI"
    let socketTypeKey = "cursor_hook"
    let iconName = "cursor"
    let executableNames = ["cursor-agent", "cursor"]
    let hookScriptName = "muxy-cursor-hook"

    private static let hooksPath = NSHomeDirectory() + "/.cursor/hooks.json"
    private static let muxyMarker = "muxy-notification-hook"

    private struct EventBinding {
        let event: String
        let argument: String
    }

    private static let bindings: [EventBinding] = [
        EventBinding(event: "stop", argument: "Stop"),
        EventBinding(event: "beforeShellExecution", argument: "PermissionRequest"),
        EventBinding(event: "beforeMCPExecution", argument: "PermissionRequest"),
    ]

    func install(hookScriptPath: String) throws {
        var settings = try Self.readSettings()
        settings["version"] = settings["version"] ?? 1
        var hooks = settings["hooks"] as? [String: Any] ?? [:]

        var changed = false
        for binding in Self.bindings {
            let command = Self.hookCommand(hookScript: hookScriptPath, argument: binding.argument)
            let existing = hooks[binding.event] as? [[String: Any]]
            if Self.muxyHookMatches(entries: existing, expectedCommand: command) { continue }
            hooks[binding.event] = Self.mergeHookArray(existing: existing, command: command)
            changed = true
        }

        guard changed else { return }
        settings["hooks"] = hooks
        try Self.writeSettings(settings)
    }

    func uninstall() throws {
        guard FileManager.default.fileExists(atPath: Self.hooksPath) else { return }
        var settings = try Self.readSettings()
        guard var hooks = settings["hooks"] as? [String: Any] else { return }

        for binding in Self.bindings {
            guard var entries = hooks[binding.event] as? [[String: Any]] else { continue }
            entries.removeAll { Self.isMuxyHookEntry($0) }
            if entries.isEmpty {
                hooks.removeValue(forKey: binding.event)
            } else {
                hooks[binding.event] = entries
            }
        }

        settings["hooks"] = hooks
        try Self.writeSettings(settings)
    }

    private static func hookCommand(hookScript: String, argument: String) -> String {
        "'\(hookScript)' \(argument) # \(muxyMarker)"
    }

    private static func muxyHookMatches(entries: [[String: Any]]?, expectedCommand: String) -> Bool {
        guard let entries else { return false }
        return entries.contains { entry in
            (entry["command"] as? String) == expectedCommand
        }
    }

    private static func mergeHookArray(existing: [[String: Any]]?, command: String) -> [[String: Any]] {
        var entries = existing ?? []
        entries.removeAll { isMuxyHookEntry($0) }
        entries.append(["command": command])
        return entries
    }

    private static func isMuxyHookEntry(_ entry: [String: Any]) -> Bool {
        guard let command = entry["command"] as? String else { return false }
        return command.contains(muxyMarker)
    }

    private static func readSettings() throws -> [String: Any] {
        guard FileManager.default.fileExists(atPath: hooksPath) else { return [:] }
        let data = try Data(contentsOf: URL(fileURLWithPath: hooksPath))
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { return [:] }
        return json
    }

    private static func writeSettings(_ settings: [String: Any]) throws {
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
