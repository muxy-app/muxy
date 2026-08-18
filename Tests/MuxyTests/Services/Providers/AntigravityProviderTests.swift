import Foundation
import Testing

@testable import Muxy

@Suite("AntigravityProvider hooks")
struct AntigravityProviderTests {
    private func commands(script: String) -> [AntigravityProvider.HookInstallationCommand] {
        AntigravityProvider.hookEvents.map {
            AntigravityProvider.HookInstallationCommand(
                settingsKey: $0.settingsKey,
                command: AntigravityProvider.hookCommand(hookScript: script, event: $0.event),
                structure: $0.structure
            )
        }
    }

    private var foreignHookEntry: [String: Any] {
        [
            "matcher": "run_command",
            "hooks": [
                [
                    "type": "command",
                    "command": "./scripts/lint.sh",
                    "timeout": 10,
                ] as [String: Any],
            ],
        ]
    }

    @Test("provider identity matches expected wire and settings ids")
    func providerIdentity() {
        let provider = AntigravityProvider()
        #expect(provider.id == "antigravity")
        #expect(provider.displayName == "Antigravity CLI")
        #expect(provider.socketTypeKey == "antigravity_hook")
        #expect(provider.iconName == "antigravity")
        #expect(provider.executableNames == ["agy", "antigravity"])
        #expect(provider.hookScriptName == "muxy-antigravity-hook")
        #expect(provider.hookScriptExtension == "sh")

        let config = provider.agentLaunchConfiguration
        #expect(config.executable == "agy")
        #expect(config.headlessArguments == [
            "--print",
            "--output-format",
            "text",
            "--dangerously-skip-permissions",
        ])
    }

    @Test("hook command embeds the event argument and muxy marker")
    func hookCommandFormat() {
        let command = AntigravityProvider.hookCommand(hookScript: "/tmp/muxy-antigravity-hook.sh", event: "Stop")
        #expect(command == "'/tmp/muxy-antigravity-hook.sh' Stop # muxy-notification-hook")
    }

    @Test("installs grouped and flat events into empty settings")
    func installsIntoEmpty() {
        let cmds = commands(script: "/tmp/hook.sh")
        let installed = AntigravityProvider.hooks(installing: cmds, into: [:])!
        let muxyConfig = installed["muxy-notify"] as? [String: Any]
        #expect(muxyConfig != nil)

        for event in AntigravityProvider.hookEvents {
            let entries = muxyConfig?[event.settingsKey] as? [[String: Any]]
            #expect(entries?.count == 1)
            let command = AntigravityProvider.hookCommand(hookScript: "/tmp/hook.sh", event: event.event)
            switch event.structure {
            case .grouped:
                #expect(AntigravityProvider.hasSingleMuxyGroupedHook(entries: entries, expectedCommand: command))
            case .flat:
                #expect(AntigravityProvider.hasSingleMuxyFlatHook(entries: entries, expectedCommand: command))
            }
        }
    }

    @Test("installing again is idempotent")
    func installIsIdempotent() {
        let cmds = commands(script: "/tmp/hook.sh")
        let installed = AntigravityProvider.hooks(installing: cmds, into: [:])!
        #expect(AntigravityProvider.hooks(installing: cmds, into: installed) == nil)
    }

    @Test("install preserves foreign top-level hooks")
    func installPreservesForeignHooks() {
        let existing: [String: Any] = [
            "lint-checker": ["PostToolUse": [foreignHookEntry]],
        ]
        let result = AntigravityProvider.hooks(installing: commands(script: "/tmp/hook.sh"), into: existing)!
        #expect(result["lint-checker"] != nil)
        #expect(result["muxy-notify"] != nil)
    }

    @Test("reinstall with a new script path replaces stale entries without duplicating")
    func reinstallReplacesStaleEntries() {
        let installed = AntigravityProvider.hooks(installing: commands(script: "/old/hook.sh"), into: [:])!
        let reinstalled = AntigravityProvider.hooks(installing: commands(script: "/new/hook.sh"), into: installed)!
        let muxyConfig = reinstalled["muxy-notify"] as? [String: Any]

        for event in AntigravityProvider.hookEvents {
            let entries = muxyConfig?[event.settingsKey] as? [[String: Any]]
            #expect(entries?.count == 1)
            let newCommand = AntigravityProvider.hookCommand(hookScript: "/new/hook.sh", event: event.event)
            switch event.structure {
            case .grouped:
                #expect(AntigravityProvider.hasSingleMuxyGroupedHook(entries: entries, expectedCommand: newCommand))
            case .flat:
                #expect(AntigravityProvider.hasSingleMuxyFlatHook(entries: entries, expectedCommand: newCommand))
            }
        }
    }

    @Test("uninstall removes muxy-notify and drops emptied container")
    func uninstallRemovesAll() {
        let installed = AntigravityProvider.hooks(installing: commands(script: "/tmp/hook.sh"), into: [:])!
        let cleaned = AntigravityProvider.hooks(uninstallingFrom: installed)
        #expect(cleaned.isEmpty)
    }

    @Test("uninstall keeps foreign hooks intact")
    func uninstallPreservesForeignHooks() {
        let existing: [String: Any] = [
            "lint-checker": ["PostToolUse": [foreignHookEntry]],
        ]
        let installed = AntigravityProvider.hooks(installing: commands(script: "/tmp/hook.sh"), into: existing)!
        let cleaned = AntigravityProvider.hooks(uninstallingFrom: installed)
        #expect(cleaned["lint-checker"] != nil)
        #expect(cleaned["muxy-notify"] == nil)
    }

    @Test("install writes hooks.json under .gemini/config")
    func installWritesHooksFile() throws {
        try withTempHome { home in
            let script = home.appendingPathComponent("muxy-antigravity-hook.sh").path
            try "#!/bin/sh\n".write(toFile: script, atomically: true, encoding: .utf8)
            let provider = AntigravityProvider(homeDirectory: home.path, pathEnvironment: "")
            try provider.install(hookScriptPath: script)

            let hookURL = home.appendingPathComponent(".gemini/config/hooks.json")
            #expect(FileManager.default.fileExists(atPath: hookURL.path))

            let data = try Data(contentsOf: hookURL)
            let json = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
            let muxyConfig = try #require(json["muxy-notify"] as? [String: Any])

            for event in AntigravityProvider.hookEvents {
                let entries = try #require(muxyConfig[event.settingsKey] as? [[String: Any]])
                #expect(entries.count == 1)
            }
        }
    }

    @Test("verify detects satisfied and needs repair states")
    func verifyDetectsTampering() throws {
        try withTempHome { home in
            let script = home.appendingPathComponent("muxy-antigravity-hook.sh").path
            try "#!/bin/sh\n".write(toFile: script, atomically: true, encoding: .utf8)
            let provider = AntigravityProvider(homeDirectory: home.path, pathEnvironment: "")

            #expect(provider.verify(hookScriptPath: script) == .needsRepair)

            try provider.install(hookScriptPath: script)
            #expect(provider.verify(hookScriptPath: script) == .satisfied)

            let hookURL = home.appendingPathComponent(".gemini/config/hooks.json")
            let data = try Data(contentsOf: hookURL)
            var json = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
            var muxyConfig = try #require(json["muxy-notify"] as? [String: Any])
            muxyConfig.removeValue(forKey: "Stop")
            json["muxy-notify"] = muxyConfig
            let modified = try JSONSerialization.data(withJSONObject: json)
            try modified.write(to: hookURL)

            #expect(provider.verify(hookScriptPath: script) == .needsRepair)

            try provider.install(hookScriptPath: "/stale/muxy-antigravity-hook.sh")
            #expect(provider.verify(hookScriptPath: script) == .needsRepair)
        }
    }

    @Test("isHookInstalled reflects marker presence")
    func isHookInstalledReflectsMarker() throws {
        try withTempHome { home in
            let script = home.appendingPathComponent("muxy-antigravity-hook.sh").path
            try "#!/bin/sh\n".write(toFile: script, atomically: true, encoding: .utf8)
            let provider = AntigravityProvider(homeDirectory: home.path, pathEnvironment: "")

            #expect(!provider.isHookInstalled())

            try provider.install(hookScriptPath: script)
            #expect(provider.isHookInstalled())

            try provider.uninstall()
            #expect(!provider.isHookInstalled())
        }
    }

    @Test("uninstall deletes the hooks.json file when emptied")
    func uninstallDeletesEmptiedFile() throws {
        try withTempHome { home in
            let script = home.appendingPathComponent("muxy-antigravity-hook.sh").path
            try "#!/bin/sh\n".write(toFile: script, atomically: true, encoding: .utf8)
            let provider = AntigravityProvider(homeDirectory: home.path, pathEnvironment: "")

            try provider.install(hookScriptPath: script)
            let hookURL = home.appendingPathComponent(".gemini/config/hooks.json")
            #expect(FileManager.default.fileExists(atPath: hookURL.path))

            try provider.uninstall()
            #expect(!FileManager.default.fileExists(atPath: hookURL.path))
        }
    }

    @Test("executable locator resolves agy from .local/bin")
    func executableLocatorResolvesFromLocalBin() throws {
        try withTempHome { home in
            let executableURL = home.appendingPathComponent(".local/bin/agy")
            try FileManager.default.createDirectory(
                at: executableURL.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            try Data().write(to: executableURL)
            try FileManager.default.setAttributes(
                [.posixPermissions: 0o755],
                ofItemAtPath: executableURL.path
            )

            let provider = AntigravityProvider(homeDirectory: home.path, pathEnvironment: "")
            let path = provider.agentCLIExecutablePath()

            #expect(path == executableURL.path)
        }
    }

    @Test("executable locator resolves agy from .gemini/antigravity-cli/bin")
    func executableLocatorResolvesFromAntigravityBin() throws {
        try withTempHome { home in
            let executableURL = home.appendingPathComponent(".gemini/antigravity-cli/bin/agy")
            try FileManager.default.createDirectory(
                at: executableURL.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            try Data().write(to: executableURL)
            try FileManager.default.setAttributes(
                [.posixPermissions: 0o755],
                ofItemAtPath: executableURL.path
            )

            let provider = AntigravityProvider(homeDirectory: home.path, pathEnvironment: "")
            let path = provider.agentCLIExecutablePath()

            #expect(path == executableURL.path)
        }
    }

    @Test("install throws and preserves file when hooks.json root is non-object JSON")
    func installThrowsOnNonObjectRoot() throws {
        try withTempHome { home in
            let configURL = home.appendingPathComponent(".gemini/config/hooks.json")
            try FileManager.default.createDirectory(
                at: configURL.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            let arrayJSON = Data("[\"unexpected\", \"array\"]".utf8)
            try arrayJSON.write(to: configURL)

            let script = home.appendingPathComponent("muxy-antigravity-hook.sh").path
            try "#!/bin/sh\n".write(toFile: script, atomically: true, encoding: .utf8)
            let provider = AntigravityProvider(homeDirectory: home.path, pathEnvironment: "")

            #expect(throws: Error.self) {
                try provider.install(hookScriptPath: script)
            }

            let contentsAfter = try Data(contentsOf: configURL)
            #expect(contentsAfter == arrayJSON)
        }
    }

    @Test("registry resolves Antigravity provider")
    @MainActor
    func registryResolvesAntigravity() {
        let registry = AIProviderRegistry(providers: [AntigravityProvider()])

        #expect(registry.notificationSource(for: "antigravity_hook") == .aiProvider("antigravity"))
        #expect(registry.iconName(forProviderID: "antigravity") == "antigravity")
        #expect(registry.iconName(for: .aiProvider("antigravity")) == "antigravity")
        #expect(registry.agentLaunchProviders.contains { $0.id == "antigravity" })
    }

    private func withTempHome(_ body: (URL) throws -> Void) throws {
        let home = FileManager.default.temporaryDirectory
            .appendingPathComponent("AntigravityProviderTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: home, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: home) }
        try body(home)
    }
}
