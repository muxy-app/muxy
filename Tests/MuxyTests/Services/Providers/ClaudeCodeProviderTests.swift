import Foundation
import Testing

@testable import Muxy

@Suite("ClaudeCodeProvider hooks")
struct ClaudeCodeProviderTests {
    private func commands(script: String) -> [(settingsKey: String, command: String)] {
        ClaudeCodeProvider.hookEvents.map {
            (settingsKey: $0.settingsKey, command: ClaudeCodeProvider.hookCommand(hookScript: script, event: $0.event))
        }
    }

    private func nonMuxyEntry(command: String) -> [[String: Any]] {
        [["matcher": "", "hooks": [["type": "command", "command": command]]]]
    }

    @Test("installs one entry per hook event into empty settings")
    func installsIntoEmpty() {
        let hooks = ClaudeCodeProvider.hooks(installing: commands(script: "/tmp/hook.sh"), into: [:])
        for key in ClaudeCodeProvider.hookEvents.map(\.settingsKey) {
            #expect((hooks?[key] as? [[String: Any]])?.count == 1)
        }
    }

    @Test("installing again is idempotent")
    func installIsIdempotent() {
        let cmds = commands(script: "/tmp/hook.sh")
        let installed = ClaudeCodeProvider.hooks(installing: cmds, into: [:])!
        #expect(ClaudeCodeProvider.hooks(installing: cmds, into: installed) == nil)
    }

    @Test("install preserves existing non-muxy hooks")
    func installPreservesForeignHooks() {
        let existing: [String: Any] = ["Stop": nonMuxyEntry(command: "echo hi")]
        let result = ClaudeCodeProvider.hooks(installing: commands(script: "/tmp/hook.sh"), into: existing)!
        #expect((result["Stop"] as? [[String: Any]])?.count == 2)
    }

    @Test("reinstall with a new script path replaces the stale entry without duplicating")
    func reinstallReplacesStaleEntry() {
        let installed = ClaudeCodeProvider.hooks(installing: commands(script: "/old/hook.sh"), into: [:])!
        let reinstalled = ClaudeCodeProvider.hooks(installing: commands(script: "/new/hook.sh"), into: installed)!
        let stop = reinstalled["Stop"] as? [[String: Any]]
        #expect(stop?.count == 1)
        let command = (stop?.first?["hooks"] as? [[String: Any]])?.first?["command"] as? String
        #expect(command?.contains("/new/hook.sh") == true)
    }

    @Test("uninstall removes every muxy entry and drops emptied keys")
    func uninstallRemovesAll() {
        let installed = ClaudeCodeProvider.hooks(installing: commands(script: "/tmp/hook.sh"), into: [:])!
        let cleaned = ClaudeCodeProvider.hooks(uninstallingFrom: installed)
        #expect(cleaned.isEmpty)
    }

    @Test("uninstall keeps foreign hooks intact")
    func uninstallPreservesForeignHooks() {
        let existing: [String: Any] = ["Stop": nonMuxyEntry(command: "echo hi")]
        let installed = ClaudeCodeProvider.hooks(installing: commands(script: "/tmp/hook.sh"), into: existing)!
        let cleaned = ClaudeCodeProvider.hooks(uninstallingFrom: installed)
        let stop = cleaned["Stop"] as? [[String: Any]]
        #expect(stop?.count == 1)
        let command = (stop?.first?["hooks"] as? [[String: Any]])?.first?["command"] as? String
        #expect(command == "echo hi")
    }
}
