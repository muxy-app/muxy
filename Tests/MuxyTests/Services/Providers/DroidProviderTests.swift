import Foundation
import Testing

@testable import Muxy

@Suite("DroidProvider hooks")
struct DroidProviderTests {
    private func commands(script: String) -> [(settingsKey: String, command: String)] {
        DroidProvider.hookEvents.map {
            (settingsKey: $0.settingsKey, command: DroidProvider.hookCommand(hookScript: script, event: $0.event))
        }
    }

    private func nonMuxyEntry(command: String) -> [[String: Any]] {
        [["matcher": "", "hooks": [["type": "command", "command": command]]]]
    }

    @Test("installs the working, waiting and idle events into empty settings")
    func installsIntoEmpty() {
        let hooks = DroidProvider.hooks(installing: commands(script: "/tmp/hook.sh"), into: [:])
        for key in ["Stop", "Notification", "UserPromptSubmit", "PreToolUse"] {
            #expect((hooks?[key] as? [[String: Any]])?.count == 1)
        }
    }

    @Test("installing again is idempotent")
    func installIsIdempotent() {
        let cmds = commands(script: "/tmp/hook.sh")
        let installed = DroidProvider.hooks(installing: cmds, into: [:])!
        #expect(DroidProvider.hooks(installing: cmds, into: installed) == nil)
    }

    @Test("install preserves existing non-muxy hooks")
    func installPreservesForeignHooks() {
        let existing: [String: Any] = ["Stop": nonMuxyEntry(command: "echo hi")]
        let result = DroidProvider.hooks(installing: commands(script: "/tmp/hook.sh"), into: existing)!
        #expect((result["Stop"] as? [[String: Any]])?.count == 2)
    }

    @Test("reinstall with a new script path replaces the stale entry without duplicating")
    func reinstallReplacesStaleEntry() {
        let installed = DroidProvider.hooks(installing: commands(script: "/old/hook.sh"), into: [:])!
        let reinstalled = DroidProvider.hooks(installing: commands(script: "/new/hook.sh"), into: installed)!
        let preToolUse = reinstalled["PreToolUse"] as? [[String: Any]]
        #expect(preToolUse?.count == 1)
        let command = (preToolUse?.first?["hooks"] as? [[String: Any]])?.first?["command"] as? String
        #expect(command?.contains("/new/hook.sh") == true)
    }

    @Test("uninstall removes every muxy entry and drops emptied keys")
    func uninstallRemovesAll() {
        let installed = DroidProvider.hooks(installing: commands(script: "/tmp/hook.sh"), into: [:])!
        let cleaned = DroidProvider.hooks(uninstallingFrom: installed)
        #expect(cleaned.isEmpty)
    }

    @Test("uninstall keeps foreign hooks intact")
    func uninstallPreservesForeignHooks() {
        let existing: [String: Any] = ["Stop": nonMuxyEntry(command: "echo hi")]
        let installed = DroidProvider.hooks(installing: commands(script: "/tmp/hook.sh"), into: existing)!
        let cleaned = DroidProvider.hooks(uninstallingFrom: installed)
        let stop = cleaned["Stop"] as? [[String: Any]]
        #expect(stop?.count == 1)
        let command = (stop?.first?["hooks"] as? [[String: Any]])?.first?["command"] as? String
        #expect(command == "echo hi")
    }
}
