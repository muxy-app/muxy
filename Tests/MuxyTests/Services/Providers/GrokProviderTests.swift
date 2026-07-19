import Foundation
import Testing

@testable import Muxy

@Suite("GrokProvider hooks")
struct GrokProviderTests {
    private func commands(script: String) -> [(settingsKey: String, command: String)] {
        GrokProvider.hookEvents.map {
            (settingsKey: $0.settingsKey, command: GrokProvider.hookCommand(hookScript: script, event: $0.event))
        }
    }

    private func nonMuxyEntry(command: String) -> [[String: Any]] {
        [["matcher": "", "hooks": [["type": "command", "command": command]]]]
    }

    @Test("provider identity matches expected wire and settings ids")
    func providerIdentity() {
        let provider = GrokProvider()
        #expect(provider.id == "grok")
        #expect(provider.displayName == "Grok")
        #expect(provider.socketTypeKey == "grok_hook")
        #expect(provider.iconName == "grok")
        #expect(provider.executableNames == ["grok"])
        #expect(provider.hookScriptName == "muxy-grok-hook")
        #expect(provider.hookScriptExtension == "sh")
    }

    @Test("hook command embeds the event argument and muxy marker")
    func hookCommandFormat() {
        let command = GrokProvider.hookCommand(hookScript: "/tmp/muxy-grok-hook.sh", event: "stop")
        #expect(command == "'/tmp/muxy-grok-hook.sh' stop # muxy-notification-hook")
    }

    @Test("installs the working, waiting and idle events into empty settings")
    func installsIntoEmpty() {
        let hooks = GrokProvider.hooks(installing: commands(script: "/tmp/hook.sh"), into: [:])
        for key in ["Stop", "Notification", "UserPromptSubmit", "PreToolUse"] {
            #expect((hooks?[key] as? [[String: Any]])?.count == 1)
        }
    }

    @Test("installing again is idempotent")
    func installIsIdempotent() {
        let cmds = commands(script: "/tmp/hook.sh")
        let installed = GrokProvider.hooks(installing: cmds, into: [:])!
        #expect(GrokProvider.hooks(installing: cmds, into: installed) == nil)
    }

    @Test("install preserves existing non-muxy hooks")
    func installPreservesForeignHooks() {
        let existing: [String: Any] = ["Stop": nonMuxyEntry(command: "echo hi")]
        let result = GrokProvider.hooks(installing: commands(script: "/tmp/hook.sh"), into: existing)!
        #expect((result["Stop"] as? [[String: Any]])?.count == 2)
    }

    @Test("reinstall with a new script path replaces the stale entry without duplicating")
    func reinstallReplacesStaleEntry() {
        let installed = GrokProvider.hooks(installing: commands(script: "/old/hook.sh"), into: [:])!
        let reinstalled = GrokProvider.hooks(installing: commands(script: "/new/hook.sh"), into: installed)!
        let preToolUse = reinstalled["PreToolUse"] as? [[String: Any]]
        #expect(preToolUse?.count == 1)
        let command = (preToolUse?.first?["hooks"] as? [[String: Any]])?.first?["command"] as? String
        #expect(command?.contains("/new/hook.sh") == true)
    }

    @Test("uninstall removes every muxy entry and drops emptied keys")
    func uninstallRemovesAll() {
        let installed = GrokProvider.hooks(installing: commands(script: "/tmp/hook.sh"), into: [:])!
        let cleaned = GrokProvider.hooks(uninstallingFrom: installed)
        #expect(cleaned.isEmpty)
    }

    @Test("uninstall keeps foreign hooks intact")
    func uninstallPreservesForeignHooks() {
        let existing: [String: Any] = ["Stop": nonMuxyEntry(command: "echo hi")]
        let installed = GrokProvider.hooks(installing: commands(script: "/tmp/hook.sh"), into: existing)!
        let cleaned = GrokProvider.hooks(uninstallingFrom: installed)
        let stop = cleaned["Stop"] as? [[String: Any]]
        #expect(stop?.count == 1)
        let command = (stop?.first?["hooks"] as? [[String: Any]])?.first?["command"] as? String
        #expect(command == "echo hi")
    }

    @Test("install writes muxy-notify.json under the injectable home hooks dir")
    func installWritesHooksFile() throws {
        try withTempHome { home in
            let script = home.appendingPathComponent("muxy-grok-hook.sh").path
            try "#!/bin/sh\n".write(toFile: script, atomically: true, encoding: .utf8)
            let provider = GrokProvider(homeDirectory: home.path, pathEnvironment: "")
            try provider.install(hookScriptPath: script)

            let hookURL = home.appendingPathComponent(".grok/hooks/muxy-notify.json")
            #expect(FileManager.default.fileExists(atPath: hookURL.path))

            let data = try Data(contentsOf: hookURL)
            let json = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
            let hooks = try #require(json["hooks"] as? [String: Any])
            for key in ["Stop", "Notification", "UserPromptSubmit", "PreToolUse"] {
                let entries = try #require(hooks[key] as? [[String: Any]])
                #expect(entries.count == 1)
                let command = (entries.first?["hooks"] as? [[String: Any]])?.first?["command"] as? String
                #expect(command?.contains(script) == true)
                #expect(command?.contains("muxy-notification-hook") == true)
            }
        }
    }

    @Test("install is a no-op when the file already matches")
    func installSkipsWhenAlreadyCurrent() throws {
        try withTempHome { home in
            let script = home.appendingPathComponent("muxy-grok-hook.sh").path
            try "#!/bin/sh\n".write(toFile: script, atomically: true, encoding: .utf8)
            let provider = GrokProvider(homeDirectory: home.path, pathEnvironment: "")
            try provider.install(hookScriptPath: script)
            let hookURL = home.appendingPathComponent(".grok/hooks/muxy-notify.json")
            let first = try Data(contentsOf: hookURL)
            try provider.install(hookScriptPath: script)
            let second = try Data(contentsOf: hookURL)
            #expect(first == second)
        }
    }

    @Test("uninstall strips muxy hooks but keeps the file and other root keys")
    func uninstallStripsHooksKeepsOtherKeys() throws {
        try withTempHome { home in
            let hooksDir = home.appendingPathComponent(".grok/hooks", isDirectory: true)
            try FileManager.default.createDirectory(at: hooksDir, withIntermediateDirectories: true)
            let hookURL = hooksDir.appendingPathComponent("muxy-notify.json")
            let preExisting: [String: Any] = [
                "version": 1,
                "hooks": [
                    "Stop": nonMuxyEntry(command: "echo foreign"),
                ],
            ]
            let data = try JSONSerialization.data(withJSONObject: preExisting, options: [.prettyPrinted, .sortedKeys])
            try data.write(to: hookURL)

            let script = home.appendingPathComponent("muxy-grok-hook.sh").path
            try "#!/bin/sh\n".write(toFile: script, atomically: true, encoding: .utf8)
            let provider = GrokProvider(homeDirectory: home.path, pathEnvironment: "")
            try provider.install(hookScriptPath: script)
            try provider.uninstall()

            #expect(FileManager.default.fileExists(atPath: hookURL.path))
            let remaining = try Data(contentsOf: hookURL)
            let json = try #require(JSONSerialization.jsonObject(with: remaining) as? [String: Any])
            #expect(json["version"] as? Int == 1)
            let hooks = try #require(json["hooks"] as? [String: Any])
            #expect(hooks["Notification"] == nil)
            let stop = try #require(hooks["Stop"] as? [[String: Any]])
            let command = (stop.first?["hooks"] as? [[String: Any]])?.first?["command"] as? String
            #expect(command == "echo foreign")
        }
    }

    @Test("uninstall with only muxy hooks removes the hooks key but keeps the file")
    func uninstallMuxyOnlyRemovesHooksKey() throws {
        try withTempHome { home in
            let script = home.appendingPathComponent("muxy-grok-hook.sh").path
            try "#!/bin/sh\n".write(toFile: script, atomically: true, encoding: .utf8)
            let provider = GrokProvider(homeDirectory: home.path, pathEnvironment: "")
            try provider.install(hookScriptPath: script)
            let hookURL = home.appendingPathComponent(".grok/hooks/muxy-notify.json")
            #expect(FileManager.default.fileExists(atPath: hookURL.path))
            try provider.uninstall()
            #expect(FileManager.default.fileExists(atPath: hookURL.path))
            let remaining = try Data(contentsOf: hookURL)
            let json = try #require(JSONSerialization.jsonObject(with: remaining) as? [String: Any])
            #expect(json["hooks"] == nil)
        }
    }

    @Test("install uses MuxyNotificationHooks.scriptPath for the shipped muxy-grok-hook resource")
    func installUsesShippedHookScriptPath() throws {
        try withTempHome { home in
            let scriptPath = try #require(
                MuxyNotificationHooks.scriptPath(named: "muxy-grok-hook", extension: "sh")
                    ?? Self.repositoryScriptPath()
            )
            #expect(scriptPath.hasSuffix("muxy-grok-hook.sh"))
            #expect(FileManager.default.fileExists(atPath: scriptPath))

            let provider = GrokProvider(homeDirectory: home.path, pathEnvironment: "")
            try provider.install(hookScriptPath: scriptPath)

            let hookURL = home.appendingPathComponent(".grok/hooks/muxy-notify.json")
            let data = try Data(contentsOf: hookURL)
            let json = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
            let hooks = try #require(json["hooks"] as? [String: Any])
            let stop = try #require(hooks["Stop"] as? [[String: Any]])
            let command = (stop.first?["hooks"] as? [[String: Any]])?.first?["command"] as? String
            #expect(command?.contains("muxy-grok-hook.sh") == true)
            #expect(command?.contains(" stop ") == true)
            #expect(command?.contains("muxy-notification-hook") == true)
        }
    }

    @Test("uninstall preserves foreign hooks in the shared file")
    func uninstallKeepsForeignEntriesOnDisk() throws {
        try withTempHome { home in
            let hooksDir = home.appendingPathComponent(".grok/hooks", isDirectory: true)
            try FileManager.default.createDirectory(at: hooksDir, withIntermediateDirectories: true)
            let hookURL = hooksDir.appendingPathComponent("muxy-notify.json")
            let preExisting: [String: Any] = [
                "hooks": [
                    "Stop": nonMuxyEntry(command: "echo foreign"),
                ],
            ]
            let data = try JSONSerialization.data(withJSONObject: preExisting, options: [.prettyPrinted, .sortedKeys])
            try data.write(to: hookURL)

            let script = home.appendingPathComponent("muxy-grok-hook.sh").path
            try "#!/bin/sh\n".write(toFile: script, atomically: true, encoding: .utf8)
            let provider = GrokProvider(homeDirectory: home.path, pathEnvironment: "")
            try provider.install(hookScriptPath: script)
            try provider.uninstall()

            #expect(FileManager.default.fileExists(atPath: hookURL.path))
            let remaining = try Data(contentsOf: hookURL)
            let json = try #require(JSONSerialization.jsonObject(with: remaining) as? [String: Any])
            let hooks = try #require(json["hooks"] as? [String: Any])
            #expect(hooks["Stop"] != nil)
            #expect(hooks["Notification"] == nil)
            let stop = try #require(hooks["Stop"] as? [[String: Any]])
            let command = (stop.first?["hooks"] as? [[String: Any]])?.first?["command"] as? String
            #expect(command == "echo foreign")
        }
    }

    @Test("isToolInstalled finds grok under injectable home paths")
    func detectsInstalledBinary() throws {
        try withTempHome { home in
            let bin = home.appendingPathComponent(".local/bin", isDirectory: true)
            try FileManager.default.createDirectory(at: bin, withIntermediateDirectories: true)
            let executable = bin.appendingPathComponent("grok")
            try "#!/bin/sh\n".write(to: executable, atomically: true, encoding: .utf8)
            try FileManager.default.setAttributes(
                [.posixPermissions: 0o755],
                ofItemAtPath: executable.path
            )
            let provider = GrokProvider(homeDirectory: home.path, pathEnvironment: "")
            #expect(provider.isToolInstalled())
        }
    }

    @Test("isToolInstalled is false when the binary is missing")
    func detectsMissingBinary() throws {
        try withTempHome { home in
            let provider = GrokProvider(homeDirectory: home.path, pathEnvironment: "")
            #expect(!provider.isToolInstalled())
        }
    }

    @Test("registry resolves grok_hook to the grok provider id and icon")
    @MainActor
    func registryResolvesGrok() {
        #expect(AIProviderRegistry.shared.notificationSource(for: "grok_hook") == .aiProvider("grok"))
        #expect(AIProviderRegistry.shared.iconName(for: .aiProvider("grok")) == "grok")
        #expect(AIProviderRegistry.shared.providers.contains(where: { $0.id == "grok" && $0.socketTypeKey == "grok_hook" }))
    }

    private func withTempHome(_ body: (URL) throws -> Void) throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("GrokProviderTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        try body(root)
    }

    private static func repositoryScriptPath() -> String? {
        let candidate = RepositoryRoot.find().appendingPathComponent("Muxy/Resources/scripts/muxy-grok-hook.sh")
        guard FileManager.default.fileExists(atPath: candidate.path) else { return nil }
        return candidate.path
    }

}
