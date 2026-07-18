import Foundation
import Testing

@testable import Muxy

@Suite("CursorProvider")
struct CursorProviderTests {
    @Test("install writes supported lifecycle hooks and removes obsolete Muxy hooks")
    func installMigratesHooks() throws {
        try withFixture { fixture in
            try fixture.writeHooks([
                "beforeShellExecution": [fixture.muxyEntry("old-shell"), fixture.foreignEntry],
                "beforeMCPExecution": [fixture.muxyEntry("old-mcp")],
            ])

            try fixture.provider.install(hookScriptPath: "/tmp/muxy-cursor-hook.sh")

            let settings = try fixture.settings()
            let hooks = try #require(settings["hooks"] as? [String: Any])
            #expect(settings["version"] as? Int == 1)
            #expect(hooks["beforeMCPExecution"] == nil)
            #expect((hooks["beforeShellExecution"] as? [[String: Any]])?.count == 1)
            #expect(fixture.command(in: hooks, event: "beforeSubmitPrompt")?.contains(" beforeSubmitPrompt ") == true)
            #expect(fixture.command(in: hooks, event: "stop")?.contains(" stop ") == true)
            #expect(fixture.command(in: hooks, event: "sessionEnd")?.contains(" sessionEnd ") == true)
        }
    }

    @Test("install is idempotent")
    func installIsIdempotent() throws {
        try withFixture { fixture in
            try fixture.provider.install(hookScriptPath: "/tmp/muxy-cursor-hook.sh")
            let first = try Data(contentsOf: fixture.hooksURL)

            try fixture.provider.install(hookScriptPath: "/tmp/muxy-cursor-hook.sh")

            #expect(try Data(contentsOf: fixture.hooksURL) == first)
        }
    }

    @Test("uninstall removes current and legacy Muxy hooks while preserving foreign hooks")
    func uninstallPreservesForeignHooks() throws {
        try withFixture { fixture in
            try fixture.writeHooks([
                "beforeSubmitPrompt": [fixture.muxyEntry("beforeSubmitPrompt"), fixture.foreignEntry],
                "stop": [fixture.muxyEntry("stop")],
                "sessionEnd": [fixture.muxyEntry("sessionEnd")],
                "beforeShellExecution": [fixture.muxyEntry("old-shell")],
            ])

            try fixture.provider.uninstall()

            let settings = try fixture.settings()
            let hooks = try #require(settings["hooks"] as? [String: Any])
            #expect((hooks["beforeSubmitPrompt"] as? [[String: Any]])?.count == 1)
            #expect(fixture.command(in: hooks, event: "beforeSubmitPrompt") == "echo foreign")
            #expect(hooks["stop"] == nil)
            #expect(hooks["sessionEnd"] == nil)
            #expect(hooks["beforeShellExecution"] == nil)
        }
    }

    @Test("uninstall does not rewrite settings without Muxy-managed hooks")
    func uninstallWithoutManagedHooksIsNoOp() throws {
        try withFixture { fixture in
            try fixture.writeHooks(["beforeSubmitPrompt": [fixture.foreignEntry]])
            let before = try Data(contentsOf: fixture.hooksURL)

            try fixture.provider.uninstall()

            #expect(try Data(contentsOf: fixture.hooksURL) == before)
        }
    }

    private func withFixture(_ body: (Fixture) throws -> Void) throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        try body(fixture)
    }

    private struct Fixture {
        let rootURL: URL
        let hooksURL: URL
        let provider: CursorProvider
        let foreignEntry: [String: Any] = ["command": "echo foreign"]

        init() throws {
            rootURL = FileManager.default.temporaryDirectory
                .appendingPathComponent("CursorProviderTests-\(UUID().uuidString)", isDirectory: true)
            hooksURL = rootURL.appendingPathComponent(".cursor/hooks.json")
            provider = CursorProvider(homeDirectory: rootURL.path)
            try FileManager.default.createDirectory(
                at: hooksURL.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
        }

        func muxyEntry(_ argument: String) -> [String: Any] {
            ["command": "'/tmp/muxy-cursor-hook.sh' \(argument) # muxy-notification-hook"]
        }

        func writeHooks(_ hooks: [String: Any]) throws {
            let data = try JSONSerialization.data(
                withJSONObject: ["version": 1, "hooks": hooks],
                options: [.prettyPrinted, .sortedKeys]
            )
            try data.write(to: hooksURL)
        }

        func settings() throws -> [String: Any] {
            let data = try Data(contentsOf: hooksURL)
            return try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        }

        func command(in hooks: [String: Any], event: String) -> String? {
            (hooks[event] as? [[String: Any]])?.first?["command"] as? String
        }

        func cleanUp() {
            try? FileManager.default.removeItem(at: rootURL)
        }
    }
}
