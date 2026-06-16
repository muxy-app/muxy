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

    @Test("install writes supported Stop hook and preserves colocated legacy user hook")
    func installWritesSupportedStopHookAndPreservesColocatedLegacyUserHook() throws {
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

        #expect(Self.commands(in: settings, event: "Stop") == [
            "'/tmp/muxy-codex-hook.sh' stop # muxy-notification-hook",
        ])
        #expect(Self.commands(in: settings, event: "Notification") == ["/usr/bin/true"])
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
        #expect(Self.commands(in: settings, event: "Notification").isEmpty)
    }

    private final class PathEnvironment {
        var value = ""
    }

    private struct Fixture {
        let rootURL: URL
        let homeURL: URL
        let hooksURL: URL

        init() throws {
            rootURL = FileManager.default.temporaryDirectory
                .appendingPathComponent("CodexProviderTests-\(UUID().uuidString)", isDirectory: true)
            homeURL = rootURL.appendingPathComponent("home", isDirectory: true)
            hooksURL = homeURL.appendingPathComponent(".codex/hooks.json")
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
        guard let hooks = settings["hooks"] as? [String: Any],
              let entries = hooks[event] as? [[String: Any]]
        else { return [] }

        return entries.reduce(into: [String]()) { commands, entry in
            guard let hooks = entry["hooks"] as? [[String: Any]] else { return }
            commands.append(contentsOf: hooks.compactMap { $0["command"] as? String })
        }
    }
}
