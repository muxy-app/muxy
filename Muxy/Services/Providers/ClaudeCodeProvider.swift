import Foundation

struct ClaudeCodeProvider: AIProviderIntegration, AIUsageProvider {
    let id = "claude"
    let displayName = "Claude Code"
    let socketTypeKey = "claude_hook"
    let iconName = "claude"
    let executableNames = ["claude"]

    private static let settingsPath = NSHomeDirectory() + "/.claude/settings.json"
    private static let muxyMarker = "muxy-notification-hook"

    func isToolInstalled() -> Bool {
        let home = NSHomeDirectory()
        let paths = [
            "\(home)/.local/bin/claude",
            "/usr/local/bin/claude",
            "/opt/homebrew/bin/claude",
        ]
        return paths.contains { FileManager.default.isExecutableFile(atPath: $0) }
    }

    func install(hookScriptPath: String) throws {
        let settings = try Self.readSettings()
        let hooks = settings["hooks"] as? [String: Any] ?? [:]

        let stopCommand = Self.hookCommand(hookScript: hookScriptPath, event: "stop")
        let notificationCommand = Self.hookCommand(hookScript: hookScriptPath, event: "notification")

        let stopMatches = Self.muxyHookMatches(entries: hooks["Stop"] as? [[String: Any]], expectedCommand: stopCommand)
        let notificationMatches = Self.muxyHookMatches(
            entries: hooks["Notification"] as? [[String: Any]],
            expectedCommand: notificationCommand
        )

        guard !stopMatches || !notificationMatches else { return }

        var updatedSettings = settings
        var updatedHooks = hooks

        let stopHook = Self.buildHookEntry(command: stopCommand)
        let notificationHook = Self.buildHookEntry(command: notificationCommand)

        updatedHooks["Stop"] = Self.mergeHookArray(existing: hooks["Stop"] as? [[String: Any]], muxyHook: stopHook)
        updatedHooks["Notification"] = Self.mergeHookArray(
            existing: hooks["Notification"] as? [[String: Any]],
            muxyHook: notificationHook
        )

        updatedSettings["hooks"] = updatedHooks
        try Self.writeSettings(updatedSettings)
    }

    func uninstall() throws {
        guard FileManager.default.fileExists(atPath: Self.settingsPath) else { return }
        var settings = try Self.readSettings()
        guard var hooks = settings["hooks"] as? [String: Any] else { return }

        for key in ["Stop", "Notification"] {
            guard var entries = hooks[key] as? [[String: Any]] else { continue }
            entries.removeAll { Self.isMuxyHookEntry($0) }
            if entries.isEmpty {
                hooks.removeValue(forKey: key)
            } else {
                hooks[key] = entries
            }
        }

        settings["hooks"] = hooks
        try Self.writeSettings(settings)
    }

    private static func hookCommand(hookScript: String, event: String) -> String {
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

    private static func readSettings() throws -> [String: Any] {
        guard FileManager.default.fileExists(atPath: settingsPath) else { return [:] }
        let data = try Data(contentsOf: URL(fileURLWithPath: settingsPath))
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { return [:] }
        return json
    }

    private static func writeSettings(_ settings: [String: Any]) throws {
        let dirPath = (settingsPath as NSString).deletingLastPathComponent
        try FileManager.default.createDirectory(atPath: dirPath, withIntermediateDirectories: true)

        let fileURL = URL(fileURLWithPath: settingsPath)
        if FileManager.default.fileExists(atPath: settingsPath) {
            let backupPath = settingsPath + ".muxy-backup"
            let backupURL = URL(fileURLWithPath: backupPath)
            try? FileManager.default.removeItem(at: backupURL)
            try FileManager.default.copyItem(at: fileURL, to: backupURL)
        }

        let data = try JSONSerialization.data(withJSONObject: settings, options: [.prettyPrinted, .sortedKeys])
        try data.write(to: fileURL, options: .atomic)
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.privateFile],
            ofItemAtPath: settingsPath
        )
    }

    private static let credentialsKeychainService = "Claude Code-credentials"
    private static let usageEndpoint = URL(string: "https://api.anthropic.com/api/oauth/usage")

    private static func credentialsFilePath(env: [String: String] = ProcessInfo.processInfo.environment) -> String {
        let base = env["CLAUDE_CONFIG_DIR"]?.trimmingCharacters(in: .whitespacesAndNewlines)
            ?? "\(NSHomeDirectory())/.claude"
        return "\(base)/.credentials.json"
    }

    func fetchUsageSnapshot() async -> AIProviderUsageSnapshot {
        await AIUsageSession.fetchSnapshot(
            provider: self,
            messages: AIUsageSessionMessages(
                missingCredentials: "Sign in to Claude",
                unauthenticated: "Sign in to Claude"
            ),
            buildRequest: {
                guard let endpoint = Self.usageEndpoint else { throw AIUsageAuthError.missingCredentials }
                let token = try readAccessToken()
                var request = URLRequest(url: endpoint)
                request.httpMethod = "GET"
                request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
                request.setValue("application/json", forHTTPHeaderField: "Accept")
                request.setValue("application/json", forHTTPHeaderField: "Content-Type")
                request.setValue("oauth-2025-04-20", forHTTPHeaderField: "anthropic-beta")
                return request
            },
            parse: ClaudeUsageParser.parseMetricRows(from:)
        )
    }

    private func readAccessToken() throws -> String {
        let env = ProcessInfo.processInfo.environment

        if let token = AIUsageTokenReader.fromEnvironment(keys: ["CLAUDE_CODE_OAUTH_TOKEN"], env: env) {
            return token
        }

        if let token = try AIUsageTokenReader.fromJSONFile(
            path: Self.credentialsFilePath(env: env),
            nestedKeyPath: ["claudeAiOauth"],
            valueKeys: ["accessToken"]
        ), !token.isEmpty {
            return token
        }

        let account = env["USER"]?.trimmingCharacters(in: .whitespacesAndNewlines)
        if let raw = AIUsageTokenReader.fromKeychain(service: Self.credentialsKeychainService, account: account),
           let data = raw.data(using: .utf8),
           let payload = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
           let oauth = payload["claudeAiOauth"] as? [String: Any],
           let token = AIUsageParserSupport.string(in: oauth, keys: ["accessToken"]),
           !token.isEmpty
        {
            return token
        }

        throw AIUsageAuthError.missingCredentials
    }
}
