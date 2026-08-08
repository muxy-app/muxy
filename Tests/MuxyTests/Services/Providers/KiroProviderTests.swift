import Foundation
import Testing

@testable import Muxy

@Suite("KiroProvider")
struct KiroProviderTests {
    private let triggers = ["UserPromptSubmit", "PreToolUse", "Stop"]

    @Test("install writes one marker-tagged entry per trigger")
    func installWritesAllTriggers() throws {
        try withFixture { fixture in
            try fixture.provider.install(hookScriptPath: fixture.hookScriptPath)

            let settings = try fixture.settings()
            #expect(settings["version"] as? String == "v1")

            let hooks = try fixture.hooks()
            #expect(hooks.count == triggers.count)

            for trigger in triggers {
                let entries = hooks.filter { $0["trigger"] as? String == trigger }
                #expect(entries.count == 1)
                let entry = try #require(entries.first)
                #expect(entry["name"] as? String == "muxy-notify-\(trigger)")

                let action = try #require(entry["action"] as? [String: Any])
                #expect(action["type"] as? String == "command")
                #expect(action["command"] as? String == KiroProvider.hookCommand(hookScript: fixture.hookScriptPath, trigger: trigger))
                #expect(entry["timeout"] as? Int == 10)
            }
        }
    }

    @Test("install is idempotent")
    func installIsIdempotent() throws {
        try withFixture { fixture in
            try fixture.provider.install(hookScriptPath: fixture.hookScriptPath)
            let first = try Data(contentsOf: fixture.hooksURL)

            try fixture.provider.install(hookScriptPath: fixture.hookScriptPath)
            let second = try Data(contentsOf: fixture.hooksURL)

            #expect(first == second)
        }
    }

    @Test("install preserves foreign hooks")
    func installPreservesForeignHooks() throws {
        try withFixture { fixture in
            try fixture.writeHooks([fixture.foreignEntry])

            try fixture.provider.install(hookScriptPath: fixture.hookScriptPath)

            let hooks = try fixture.hooks()
            #expect(hooks.count == triggers.count + 1)
            #expect(hooks.contains { entry in
                (entry["action"] as? [String: Any])?["command"] as? String == "echo foreign"
            })
        }
    }

    @Test("reinstall replaces stale entries without duplicating")
    func reinstallReplacesStaleEntries() throws {
        try withFixture { fixture in
            let oldPath = "/old/muxy-kiro-hook.sh"
            let newPath = "/new/muxy-kiro-hook.sh"

            try fixture.provider.install(hookScriptPath: oldPath)
            try fixture.provider.install(hookScriptPath: newPath)

            let hooks = try fixture.hooks()
            #expect(hooks.count == triggers.count)
            #expect(hooks.allSatisfy { entry in
                ((entry["action"] as? [String: Any])?["command"] as? String)?.contains(newPath) == true
            })
            #expect(!hooks.contains { entry in
                ((entry["action"] as? [String: Any])?["command"] as? String)?.contains(oldPath) == true
            })
        }
    }

    @Test("uninstall removes only Muxy entries")
    func uninstallRemovesOnlyMuxyEntries() throws {
        try withFixture { fixture in
            try fixture.writeHooks([fixture.foreignEntry])
            try fixture.provider.install(hookScriptPath: fixture.hookScriptPath)

            try fixture.provider.uninstall()

            let hooks = try fixture.hooks()
            #expect(hooks.count == 1)
            #expect((hooks.first?["action"] as? [String: Any])?["command"] as? String == "echo foreign")
        }
    }

    @Test("uninstall deletes the file when emptied")
    func uninstallDeletesEmptiedFile() throws {
        try withFixture { fixture in
            try fixture.provider.install(hookScriptPath: fixture.hookScriptPath)

            try fixture.provider.uninstall()

            #expect(!FileManager.default.fileExists(atPath: fixture.hooksURL.path))
        }
    }

    @Test("uninstall is no-op when only foreign hooks exist")
    func uninstallNoOpWithOnlyForeignHooks() throws {
        try withFixture { fixture in
            try fixture.writeHooks([fixture.foreignEntry])
            let before = try Data(contentsOf: fixture.hooksURL)

            try fixture.provider.uninstall()
            let after = try Data(contentsOf: fixture.hooksURL)

            #expect(before == after)
        }
    }

    @Test("verify detects satisfied and needs repair states")
    func verifyDetectsTampering() throws {
        try withFixture { fixture in
            #expect(fixture.provider.verify(hookScriptPath: fixture.hookScriptPath) == .needsRepair)

            try fixture.provider.install(hookScriptPath: fixture.hookScriptPath)
            #expect(fixture.provider.verify(hookScriptPath: fixture.hookScriptPath) == .satisfied)

            var hooks = try fixture.hooks()
            hooks.removeAll { $0["trigger"] as? String == "Stop" }
            try fixture.writeHooks(hooks)
            #expect(fixture.provider.verify(hookScriptPath: fixture.hookScriptPath) == .needsRepair)

            try fixture.provider.install(hookScriptPath: "/stale/muxy-kiro-hook.sh")
            #expect(fixture.provider.verify(hookScriptPath: fixture.hookScriptPath) == .needsRepair)
        }
    }

    @Test("isHookInstalled reflects marker presence")
    func isHookInstalledReflectsMarker() throws {
        try withFixture { fixture in
            #expect(!fixture.provider.isHookInstalled())

            try fixture.provider.install(hookScriptPath: fixture.hookScriptPath)
            #expect(fixture.provider.isHookInstalled())

            try fixture.provider.uninstall()
            #expect(!fixture.provider.isHookInstalled())
        }
    }

    @Test("identity, config paths, and launch configuration")
    func identityAndLaunchConfig() throws {
        try withFixture { fixture in
            #expect(fixture.provider.id == "kiro")
            #expect(fixture.provider.displayName == "Kiro CLI")
            #expect(fixture.provider.socketTypeKey == "kiro_hook")
            #expect(fixture.provider.iconName == "kiro")
            #expect(fixture.provider.executableNames == ["kiro-cli"])
            #expect(fixture.provider.hookScriptName == "muxy-kiro-hook")
            #expect(fixture.provider.configPaths == [fixture.hooksURL.path])

            let config = fixture.provider.agentLaunchConfiguration
            #expect(config.executable == "kiro-cli")
            #expect(config.headlessArguments == ["chat", "--no-interactive"])
        }
    }

    @Test("executable locator resolves fixture kiro-cli from local bin")
    func executableLocatorResolvesFromLocalBin() throws {
        try withFixture { fixture in
            let executableURL = fixture.rootURL.appendingPathComponent(".local/bin/kiro-cli")
            try FileManager.default.createDirectory(
                at: executableURL.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            try Data().write(to: executableURL)
            try FileManager.default.setAttributes(
                [.posixPermissions: 0o755],
                ofItemAtPath: executableURL.path
            )

            let path = fixture.provider.agentCLIExecutablePath()

            #expect(path == executableURL.path)
        }
    }

    @Test("registry resolves Kiro provider")
    @MainActor
    func registryResolvesKiro() {
        let registry = AIProviderRegistry(providers: [KiroProvider()])

        #expect(registry.notificationSource(for: "kiro_hook") == .aiProvider("kiro"))
        #expect(registry.iconName(forProviderID: "kiro") == "kiro")
        #expect(registry.iconName(for: .aiProvider("kiro")) == "kiro")
        #expect(registry.agentLaunchProviders.contains { $0.id == "kiro" })
    }

    private func withFixture(_ body: (Fixture) throws -> Void) throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        try body(fixture)
    }

    private struct Fixture {
        let rootURL: URL
        let hooksURL: URL
        let provider: KiroProvider
        let hookScriptPath: String

        var foreignEntry: [String: Any] {
            [
                "name": "foreign-hook",
                "trigger": "UserPromptSubmit",
                "action": ["type": "command", "command": "echo foreign"] as [String: Any],
                "timeout": 5,
            ]
        }

        init() throws {
            rootURL = FileManager.default.temporaryDirectory
                .appendingPathComponent("KiroProviderTests-\(UUID().uuidString)", isDirectory: true)
            hooksURL = rootURL.appendingPathComponent(".kiro/hooks/muxy-notify.json")
            provider = KiroProvider(homeDirectory: rootURL.path)
            hookScriptPath = "/tmp/muxy-kiro-hook.sh"
            try FileManager.default.createDirectory(
                at: hooksURL.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
        }

        func writeHooks(_ hooks: [[String: Any]], version: String = "v1") throws {
            let data = try JSONSerialization.data(
                withJSONObject: ["version": version, "hooks": hooks],
                options: [.prettyPrinted, .sortedKeys]
            )
            try data.write(to: hooksURL)
        }

        func settings() throws -> [String: Any] {
            let data = try Data(contentsOf: hooksURL)
            return try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        }

        func hooks() throws -> [[String: Any]] {
            let settings = try settings()
            return try #require(settings["hooks"] as? [[String: Any]])
        }

        func cleanUp() {
            try? FileManager.default.removeItem(at: rootURL)
        }
    }
}
