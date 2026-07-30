import Foundation
import Testing

@testable import Muxy

@Suite("CopilotProvider hooks")
struct CopilotProviderTests {
    @Test("provider identity matches expected wire and settings ids")
    func providerIdentity() {
        let provider = CopilotProvider()
        #expect(provider.id == "copilot")
        #expect(provider.displayName == "GitHub Copilot")
        #expect(provider.socketTypeKey == "copilot_hook")
        #expect(provider.iconName == "copilot")
        #expect(provider.executableNames == ["copilot"])
        #expect(provider.hookScriptName == "muxy-copilot-hook")
        #expect(provider.hookScriptExtension == "sh")
    }

    @Test("hook command embeds the event argument and muxy marker")
    func hookCommandFormat() {
        let command = CopilotProvider.hookCommand(hookScript: "/tmp/muxy-copilot-hook.sh", argument: "stop")
        #expect(command == "'/tmp/muxy-copilot-hook.sh' stop # muxy-notification-hook")
    }

    @Test("install writes version and managed lifecycle hooks into empty settings")
    func installWritesHooksFile() throws {
        try withFixture { fixture in
            try fixture.provider.install(hookScriptPath: "/tmp/muxy-copilot-hook.sh")

            let settings = try fixture.settings()
            #expect(settings["version"] as? Int == 1)
            let hooks = try #require(settings["hooks"] as? [String: Any])

            for binding in CopilotProvider.bindings {
                let bash = fixture.bash(in: hooks, event: binding.settingsKey)
                #expect(bash?.contains(binding.argument) == true)
                #expect(bash?.contains("muxy-notification-hook") == true)
                let entries = try #require(hooks[binding.settingsKey] as? [[String: Any]])
                #expect(entries.count == 1)
                #expect(entries.first?["type"] as? String == "command")
                #expect(entries.first?["timeoutSec"] as? Int == 10)
            }
            #expect(hooks["permissionRequest"] == nil)
        }
    }

    @Test("install is idempotent")
    func installIsIdempotent() throws {
        try withFixture { fixture in
            try fixture.provider.install(hookScriptPath: "/tmp/muxy-copilot-hook.sh")
            let first = try Data(contentsOf: fixture.hooksURL)

            try fixture.provider.install(hookScriptPath: "/tmp/muxy-copilot-hook.sh")

            #expect(try Data(contentsOf: fixture.hooksURL) == first)
        }
    }

    @Test("install preserves foreign hooks and replaces stale Muxy entries")
    func installPreservesForeignAndReplacesStale() throws {
        try withFixture { fixture in
            try fixture.writeHooks([
                "agentStop": [
                    fixture.muxyEntry("stop", script: "/old/muxy-copilot-hook.sh"),
                    fixture.foreignEntry,
                ],
                "permissionRequest": [
                    fixture.muxyEntry("permission-request"),
                    fixture.foreignEntry,
                ],
            ])

            try fixture.provider.install(hookScriptPath: "/tmp/muxy-copilot-hook.sh")

            let hooks = try #require(try fixture.settings()["hooks"] as? [String: Any])
            let stopEntries = try #require(hooks["agentStop"] as? [[String: Any]])
            #expect(stopEntries.count == 2)
            let bashCommands = stopEntries.compactMap { $0["bash"] as? String }
            #expect(bashCommands.contains("echo foreign"))
            #expect(bashCommands.contains { $0.contains("/tmp/muxy-copilot-hook.sh") && $0.contains(" stop ") })
            #expect(!bashCommands.contains { $0.contains("/old/muxy-copilot-hook.sh") })
            let permissionEntries = try #require(hooks["permissionRequest"] as? [[String: Any]])
            #expect(permissionEntries.count == 1)
            #expect(permissionEntries.first?["bash"] as? String == "echo foreign")
        }
    }

    @Test("verify is satisfied after install and needs repair when incomplete")
    func verifyStates() throws {
        try withFixture { fixture in
            let script = "/tmp/muxy-copilot-hook.sh"
            #expect(fixture.provider.verify(hookScriptPath: script) == .needsRepair)

            try fixture.provider.install(hookScriptPath: script)
            #expect(fixture.provider.verify(hookScriptPath: script) == .satisfied)

            try fixture.writeHooks([
                "agentStop": [fixture.muxyEntry("stop")],
            ])
            #expect(fixture.provider.verify(hookScriptPath: script) == .needsRepair)
        }
    }

    @Test("verify needs repair when hook file version drifts")
    func verifyNeedsRepairForWrongVersion() throws {
        try withFixture { fixture in
            let script = "/tmp/muxy-copilot-hook.sh"
            try fixture.provider.install(hookScriptPath: script)
            #expect(fixture.provider.verify(hookScriptPath: script) == .satisfied)

            var settings = try fixture.settings()
            settings["version"] = 2
            try fixture.writeSettings(settings)
            #expect(fixture.provider.verify(hookScriptPath: script) == .needsRepair)

            try fixture.provider.install(hookScriptPath: script)
            #expect(try fixture.settings()["version"] as? Int == 1)
            #expect(fixture.provider.verify(hookScriptPath: script) == .satisfied)
        }
    }

    @Test("verify needs repair when timeout or type drifts")
    func verifyNeedsRepairForEntryShapeDrift() throws {
        try withFixture { fixture in
            let script = "/tmp/muxy-copilot-hook.sh"
            try fixture.provider.install(hookScriptPath: script)

            var hooks = try #require(try fixture.settings()["hooks"] as? [String: Any])
            var stop = try #require(hooks["agentStop"] as? [[String: Any]])
            stop[0]["timeoutSec"] = 99
            hooks["agentStop"] = stop
            try fixture.writeHooks(hooks)
            #expect(fixture.provider.verify(hookScriptPath: script) == .needsRepair)

            try fixture.provider.install(hookScriptPath: script)
            #expect(fixture.provider.verify(hookScriptPath: script) == .satisfied)
            let repaired = try #require(
                try fixture.settings()["hooks"] as? [String: Any]
            )
            let repairedStop = try #require(repaired["agentStop"] as? [[String: Any]])
            #expect(repairedStop.first?["timeoutSec"] as? Int == 10)
            #expect(repairedStop.first?["type"] as? String == "command")
        }
    }

    @Test("uninstall removes Muxy hooks while preserving foreign hooks")
    func uninstallPreservesForeignHooks() throws {
        try withFixture { fixture in
            try fixture.provider.install(hookScriptPath: "/tmp/muxy-copilot-hook.sh")
            var hooks = try #require(try fixture.settings()["hooks"] as? [String: Any])
            var stop = try #require(hooks["agentStop"] as? [[String: Any]])
            stop.append(fixture.foreignEntry)
            hooks["agentStop"] = stop
            try fixture.writeHooks(hooks)

            try fixture.provider.uninstall()

            let remaining = try #require(try fixture.settings()["hooks"] as? [String: Any])
            #expect(remaining["userPromptSubmitted"] == nil)
            #expect(remaining["preToolUse"] == nil)
            #expect(remaining["permissionRequest"] == nil)
            #expect(remaining["notification"] == nil)
            #expect(remaining["sessionEnd"] == nil)
            #expect(remaining["errorOccurred"] == nil)
            let stopEntries = try #require(remaining["agentStop"] as? [[String: Any]])
            #expect(stopEntries.count == 1)
            #expect(stopEntries.first?["bash"] as? String == "echo foreign")
        }
    }

    @Test("uninstall is a no-op without managed hooks")
    func uninstallWithoutManagedHooksIsNoOp() throws {
        try withFixture { fixture in
            try fixture.writeHooks(["agentStop": [fixture.foreignEntry]])
            let before = try Data(contentsOf: fixture.hooksURL)

            try fixture.provider.uninstall()

            #expect(try Data(contentsOf: fixture.hooksURL) == before)
        }
    }

    @Test("respects injectable copilot home override")
    func usesCopilotHomeOverride() throws {
        try withFixture { fixture in
            let customHome = fixture.rootURL.appendingPathComponent("custom-copilot")
            let environmentHomePath = fixture.rootURL.appendingPathComponent("environment-home").path
            let provider = CopilotProvider(
                homeDirectory: fixture.rootURL.path,
                pathEnvironment: "",
                copilotHomeEnvironment: { environmentHomePath },
                copilotHome: customHome.path
            )
            try provider.install(hookScriptPath: "/tmp/muxy-copilot-hook.sh")

            let path = customHome.appendingPathComponent("hooks/muxy-notify.json").path
            #expect(FileManager.default.fileExists(atPath: path))
            #expect(provider.configPaths == [path])
            #expect(provider.isHookInstalled())
        }
    }

    @Test("respects Copilot home resolved from the login shell environment")
    func usesCopilotHomeEnvironment() throws {
        try withFixture { fixture in
            let customHome = fixture.rootURL.appendingPathComponent("shell-copilot")
            let provider = CopilotProvider(
                homeDirectory: fixture.rootURL.path,
                pathEnvironment: "",
                copilotHomeEnvironment: { customHome.path }
            )

            try provider.install(hookScriptPath: "/tmp/muxy-copilot-hook.sh")

            let path = customHome.appendingPathComponent("hooks/muxy-notify.json").path
            #expect(FileManager.default.fileExists(atPath: path))
            #expect(provider.configPaths == [path])
        }
    }

    @Test("registry exposes copilot provider socket type")
    @MainActor
    func registryIncludesCopilot() {
        #expect(AIProviderRegistry.shared.providers.contains(where: {
            $0.id == "copilot" && $0.socketTypeKey == "copilot_hook"
        }))
    }

    private func withFixture(_ body: (Fixture) throws -> Void) throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        try body(fixture)
    }

    private struct Fixture {
        let rootURL: URL
        let hooksURL: URL
        let provider: CopilotProvider
        let foreignEntry: [String: Any] = [
            "type": "command",
            "bash": "echo foreign",
            "timeoutSec": 5,
        ]

        init() throws {
            rootURL = FileManager.default.temporaryDirectory
                .appendingPathComponent("CopilotProviderTests-\(UUID().uuidString)", isDirectory: true)
            let copilotHome = rootURL.appendingPathComponent(".copilot")
            hooksURL = copilotHome.appendingPathComponent("hooks/muxy-notify.json")
            provider = CopilotProvider(
                homeDirectory: rootURL.path,
                pathEnvironment: "",
                copilotHome: copilotHome.path
            )
            try FileManager.default.createDirectory(
                at: hooksURL.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
        }

        func muxyEntry(_ argument: String, script: String = "/tmp/muxy-copilot-hook.sh") -> [String: Any] {
            [
                "type": "command",
                "bash": "'\(script)' \(argument) # muxy-notification-hook",
                "timeoutSec": 10,
            ]
        }

        func writeHooks(_ hooks: [String: Any]) throws {
            try writeSettings(["version": 1, "hooks": hooks])
        }

        func writeSettings(_ settings: [String: Any]) throws {
            let data = try JSONSerialization.data(
                withJSONObject: settings,
                options: [.prettyPrinted, .sortedKeys]
            )
            try data.write(to: hooksURL)
        }

        func settings() throws -> [String: Any] {
            let data = try Data(contentsOf: hooksURL)
            return try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        }

        func bash(in hooks: [String: Any], event: String) -> String? {
            (hooks[event] as? [[String: Any]])?.first?["bash"] as? String
        }

        func cleanUp() {
            try? FileManager.default.removeItem(at: rootURL)
        }
    }
}
