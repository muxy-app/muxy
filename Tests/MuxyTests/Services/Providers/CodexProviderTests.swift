import Foundation
import Testing

@testable import Muxy

@Suite("CodexProvider")
struct CodexProviderTests {
    @Test("isToolInstalled checks npm global bin")
    func isToolInstalledFromNpmGlobalBin() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let executableURL = fixture.homeURL.appendingPathComponent(".npm-global/bin/codex")
        try fixture.makeExecutable(at: executableURL)

        #expect(fixture.provider().isToolInstalled())
    }

    @Test("isToolInstalled checks PATH entries")
    func isToolInstalledFromPath() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let binURL = fixture.rootURL.appendingPathComponent("custom-bin")
        let executableURL = binURL.appendingPathComponent("codex")
        try fixture.makeExecutable(at: executableURL)

        #expect(fixture.provider(pathEnvironment: binURL.path).isToolInstalled())
    }

    @Test("isToolInstalled evaluates PATH at call time")
    func isToolInstalledUsesCurrentPathEnvironment() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let pathEnvironment = PathEnvironment()
        let provider = CodexProvider(
            homeDirectory: fixture.homeURL.path,
            pathEnvironment: { pathEnvironment.value },
            hooksPath: fixture.hooksURL.path
        )

        let binURL = fixture.rootURL.appendingPathComponent("late-bin")
        let executableURL = binURL.appendingPathComponent("codex")
        try fixture.makeExecutable(at: executableURL)
        pathEnvironment.value = binURL.path

        #expect(provider.isToolInstalled())
    }

    @Test("isToolInstalled returns false when no candidate exists")
    func isToolInstalledReturnsFalseWhenMissing() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        #expect(!fixture.provider().isToolInstalled())
    }

    @Test("install writes supported hooks and preserves colocated legacy user hook")
    func installWritesSupportedHooksAndPreservesColocatedLegacyUserHook() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        try fixture.writeSettings([
            "hooks": [
                "Notification": [
                    [
                        "hooks": [
                            [
                                "type": "command",
                                "command": "'/old/muxy-codex-hook.sh' notification # muxy-notification-hook",
                            ],
                            [
                                "type": "command",
                                "command": "/usr/bin/true",
                            ],
                        ],
                    ],
                ],
            ],
        ])

        try fixture.provider().install(hookScriptPath: "/tmp/muxy-codex-hook.sh")
        let settings = try fixture.readSettings()

        #expect(Self.commands(in: settings, event: "UserPromptSubmit") == [
            "'/tmp/muxy-codex-hook.sh' user-prompt-submit # muxy-notification-hook",
        ])
        #expect(Self.commands(in: settings, event: "PreToolUse") == [
            "'/tmp/muxy-codex-hook.sh' pre-tool-use # muxy-notification-hook",
        ])
        #expect(Self.commands(in: settings, event: "PermissionRequest") == [
            "'/tmp/muxy-codex-hook.sh' permission-request # muxy-notification-hook",
        ])
        #expect(Self.commands(in: settings, event: "Stop") == [
            "'/tmp/muxy-codex-hook.sh' stop # muxy-notification-hook",
        ])
        for event in ["UserPromptSubmit", "PreToolUse", "PermissionRequest", "Stop"] {
            #expect(Self.timeouts(in: settings, event: event) == [10])
        }
        #expect(Self.commands(in: settings, event: "SessionStart").isEmpty)
        #expect(Self.commands(in: settings, event: "Notification") == ["/usr/bin/true"])
    }

    @Test("installing again is idempotent")
    func installIsIdempotent() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        try fixture.provider().install(hookScriptPath: "/tmp/muxy-codex-hook.sh")
        let firstInstall = try fixture.readSettings()

        try fixture.provider().install(hookScriptPath: "/tmp/muxy-codex-hook.sh")
        let secondInstall = try fixture.readSettings()

        #expect(Self.commands(in: secondInstall, event: "UserPromptSubmit") == Self.commands(in: firstInstall, event: "UserPromptSubmit"))
        #expect(Self.commands(in: secondInstall, event: "PreToolUse") == Self.commands(in: firstInstall, event: "PreToolUse"))
        #expect(Self.commands(in: secondInstall, event: "PermissionRequest") == Self.commands(in: firstInstall, event: "PermissionRequest"))
        #expect(Self.commands(in: secondInstall, event: "Stop") == Self.commands(in: firstInstall, event: "Stop"))
    }

    @Test("install does not create hooks.json when config.toml contains hooks")
    func installSkipsConflictingRepresentation() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        try fixture.writeConfig(
            """
            [[hooks.SessionStart]]

            [[hooks.SessionStart.hooks]]
            type = "command"
            command = "/usr/bin/true"
            """
        )

        #expect(throws: CodexProviderError.inlineHooksConfigured(fixture.configURL.path)) {
            try fixture.provider().install(hookScriptPath: "/tmp/muxy-codex-hook.sh")
        }
        #expect(!FileManager.default.fileExists(atPath: fixture.hooksURL.path))
    }

    @Test("install ignores hooks.state metadata in config.toml")
    func installAllowsStateOnlyConfig() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        try fixture.writeConfig(
            """
            [hooks.state."/tmp/hooks.json:stop:0:0"]
            enabled = false
            """
        )

        try fixture.provider().install(hookScriptPath: "/tmp/muxy-codex-hook.sh")

        #expect(Self.commands(in: try fixture.readSettings(), event: "Stop") == [
            "'/tmp/muxy-codex-hook.sh' stop # muxy-notification-hook",
        ])
    }

    @Test("install removes existing Muxy JSON hooks when config.toml contains hooks")
    func installRemovesExistingConflictingHooks() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        try fixture.provider().install(hookScriptPath: "/tmp/muxy-codex-hook.sh")
        try fixture.writeConfig("[[hooks.Stop]]")

        #expect(throws: CodexProviderError.inlineHooksConfigured(fixture.configURL.path)) {
            try fixture.provider().install(hookScriptPath: "/tmp/muxy-codex-hook.sh")
        }
        #expect(Self.commands(in: try fixture.readSettings(), event: "Stop").isEmpty)
    }

    @Test("uninstall removes Muxy hooks and preserves colocated user hook")
    func uninstallRemovesMuxyHooksAndPreservesColocatedUserHook() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        try fixture.writeSettings([
            "hooks": [
                "Stop": [
                    [
                        "hooks": [
                            [
                                "type": "command",
                                "command": "'/tmp/muxy-codex-hook.sh' stop # muxy-notification-hook",
                            ],
                            [
                                "type": "command",
                                "command": "/usr/bin/true",
                            ],
                        ],
                    ],
                ],
                "UserPromptSubmit": [
                    [
                        "hooks": [
                            [
                                "type": "command",
                                "command": "'/tmp/muxy-codex-hook.sh' user-prompt-submit # muxy-notification-hook",
                            ],
                        ],
                    ],
                ],
                "PreToolUse": [
                    [
                        "hooks": [
                            [
                                "type": "command",
                                "command": "'/tmp/muxy-codex-hook.sh' pre-tool-use # muxy-notification-hook",
                            ],
                        ],
                    ],
                ],
                "PermissionRequest": [
                    [
                        "hooks": [
                            [
                                "type": "command",
                                "command": "'/tmp/muxy-codex-hook.sh' permission-request # muxy-notification-hook",
                            ],
                        ],
                    ],
                ],
                "Notification": [
                    [
                        "hooks": [
                            [
                                "type": "command",
                                "command": "'/tmp/muxy-codex-hook.sh' notification # muxy-notification-hook",
                            ],
                        ],
                    ],
                ],
            ],
        ])

        try fixture.provider().uninstall()
        let settings = try fixture.readSettings()

        #expect(Self.commands(in: settings, event: "Stop") == ["/usr/bin/true"])
        #expect(Self.commands(in: settings, event: "UserPromptSubmit").isEmpty)
        #expect(Self.commands(in: settings, event: "PreToolUse").isEmpty)
        #expect(Self.commands(in: settings, event: "PermissionRequest").isEmpty)
        #expect(Self.commands(in: settings, event: "Notification").isEmpty)
    }

    private final class PathEnvironment {
        var value = ""
    }

    private struct Fixture {
        let rootURL: URL
        let homeURL: URL
        let hooksURL: URL
        let configURL: URL

        init() throws {
            rootURL = FileManager.default.temporaryDirectory
                .appendingPathComponent("CodexProviderTests-\(UUID().uuidString)", isDirectory: true)
            homeURL = rootURL.appendingPathComponent("home", isDirectory: true)
            hooksURL = homeURL.appendingPathComponent(".codex/hooks.json")
            configURL = homeURL.appendingPathComponent(".codex/config.toml")
            try FileManager.default.createDirectory(at: homeURL, withIntermediateDirectories: true)
        }

        func provider(pathEnvironment: String = "") -> CodexProvider {
            CodexProvider(
                homeDirectory: homeURL.path,
                pathEnvironment: pathEnvironment,
                hooksPath: hooksURL.path
            )
        }

        func makeExecutable(at url: URL) throws {
            try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try Data().write(to: url)
            try FileManager.default.setAttributes(
                [.posixPermissions: FilePermissions.executable],
                ofItemAtPath: url.path
            )
        }

        func writeSettings(_ settings: [String: Any]) throws {
            try FileManager.default.createDirectory(
                at: hooksURL.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            let data = try JSONSerialization.data(withJSONObject: settings, options: [.prettyPrinted, .sortedKeys])
            try data.write(to: hooksURL, options: .atomic)
        }

        func writeConfig(_ config: String) throws {
            try FileManager.default.createDirectory(
                at: configURL.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            try config.write(to: configURL, atomically: true, encoding: .utf8)
        }

        func readSettings() throws -> [String: Any] {
            let data = try Data(contentsOf: hooksURL)
            guard let settings = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { return [:] }
            return settings
        }

        func cleanUp() {
            try? FileManager.default.removeItem(at: rootURL)
        }
    }

    private static func commands(in settings: [String: Any], event: String) -> [String] {
        hooks(in: settings, event: event).compactMap { $0["command"] as? String }
    }

    private static func timeouts(in settings: [String: Any], event: String) -> [Int] {
        hooks(in: settings, event: event).compactMap { ($0["timeout"] as? NSNumber)?.intValue }
    }

    private static func hooks(in settings: [String: Any], event: String) -> [[String: Any]] {
        guard let hooks = settings["hooks"] as? [String: Any],
              let entries = hooks[event] as? [[String: Any]]
        else { return [] }

        return entries.reduce(into: [[String: Any]]()) { result, entry in
            guard let entryHooks = entry["hooks"] as? [[String: Any]] else { return }
            result.append(contentsOf: entryHooks)
        }
    }
}
