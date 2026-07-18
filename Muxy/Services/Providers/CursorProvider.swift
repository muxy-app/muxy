import Foundation

struct CursorProvider: AIProviderIntegration, AIAgentLaunchProvider {
    let id = "cursor"
    let displayName = "Cursor CLI"
    let socketTypeKey = "cursor_hook"
    let iconName = "cursor"
    let executableNames = ["cursor-agent", "cursor"]
    let hookScriptName = "muxy-cursor-hook"

    var agentLaunchConfiguration: AIAgentLaunchConfiguration {
        AIAgentLaunchConfiguration(
            executable: "cursor-agent",
            headlessArguments: ["--print", "--output-format", "text"]
        )
    }

    private static let muxyMarker = "muxy-notification-hook"
    private let homeDirectory: String

    init(homeDirectory: String = NSHomeDirectory()) {
        self.homeDirectory = homeDirectory
    }

    func agentCLIExecutablePath() -> String? {
        ProviderExecutableLocator.executablePath(
            names: ["cursor-agent"],
            homeDirectory: homeDirectory,
            pathEnvironment: LoginShellPath.current,
            includeSystemWide: homeDirectory == NSHomeDirectory()
        )
    }

    private var hooksPath: String {
        homeDirectory + "/.cursor/hooks.json"
    }

    private struct EventBinding {
        let event: String
        let argument: String
    }

    private static let bindings: [EventBinding] = [
        EventBinding(event: "beforeSubmitPrompt", argument: "beforeSubmitPrompt"),
        EventBinding(event: "stop", argument: "stop"),
        EventBinding(event: "sessionEnd", argument: "sessionEnd"),
    ]
    private static let removableEvents = bindings.map(\.event) + ["beforeShellExecution", "beforeMCPExecution"]

    func isHookInstalled() -> Bool {
        ClaudeCodeProvider.fileContainsMuxyMarker(at: hooksPath)
    }

    func install(hookScriptPath: String) throws {
        var settings = try Self.readSettings(at: hooksPath)
        settings["version"] = settings["version"] ?? 1
        var hooks = settings["hooks"] as? [String: Any] ?? [:]

        var changed = false
        let installedEvents = Set(Self.bindings.map(\.event))
        for event in Self.removableEvents where !installedEvents.contains(event) {
            guard var entries = hooks[event] as? [[String: Any]] else { continue }
            let initialCount = entries.count
            entries.removeAll { Self.isMuxyHookEntry($0) }
            guard entries.count != initialCount else { continue }
            changed = true
            if entries.isEmpty {
                hooks.removeValue(forKey: event)
            } else {
                hooks[event] = entries
            }
        }
        for binding in Self.bindings {
            let command = Self.hookCommand(hookScript: hookScriptPath, argument: binding.argument)
            let existing = hooks[binding.event] as? [[String: Any]]
            if Self.muxyHookMatches(entries: existing, expectedCommand: command) {
                continue
            }
            hooks[binding.event] = Self.mergeHookArray(existing: existing, command: command)
            changed = true
        }

        guard changed else { return }
        settings["hooks"] = hooks
        try Self.writeSettings(settings, at: hooksPath)
    }

    func uninstall() throws {
        guard FileManager.default.fileExists(atPath: hooksPath) else { return }
        guard isHookInstalled() else { return }
        var settings = try Self.readSettings(at: hooksPath)
        guard var hooks = settings["hooks"] as? [String: Any] else { return }

        for event in Self.removableEvents {
            guard var entries = hooks[event] as? [[String: Any]] else { continue }
            entries.removeAll { Self.isMuxyHookEntry($0) }
            if entries.isEmpty {
                hooks.removeValue(forKey: event)
            } else {
                hooks[event] = entries
            }
        }

        settings["hooks"] = hooks
        try Self.writeSettings(settings, at: hooksPath)
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

    private static func readSettings(at path: String) throws -> [String: Any] {
        guard FileManager.default.fileExists(atPath: path) else { return [:] }
        let data = try Data(contentsOf: URL(fileURLWithPath: path))
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { return [:] }
        return json
    }

    private static func writeSettings(_ settings: [String: Any], at path: String) throws {
        let dirPath = (path as NSString).deletingLastPathComponent
        try FileManager.default.createDirectory(atPath: dirPath, withIntermediateDirectories: true)

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
