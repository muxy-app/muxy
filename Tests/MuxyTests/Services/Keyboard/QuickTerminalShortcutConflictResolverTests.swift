import Testing

@testable import Muxy

@Suite("QuickTerminalShortcutConflictResolver")
@MainActor
struct QuickTerminalShortcutConflictResolverTests {
    @Test("app shortcut reset reports a Quick Terminal default conflict")
    func appShortcutResetConflict() throws {
        let binding = try #require(KeyBinding.defaults.first)
        let virtualKeyCode = try #require(KeyCombo.virtualKeyCode(for: binding.combo.key))
        let shortcut = QuickTerminalShortcut.keyCombo(binding.combo, virtualKeyCode: virtualKeyCode)

        let message = QuickTerminalShortcutConflictResolver.appShortcutResetConflictMessage(
            for: binding.action,
            shortcut: shortcut
        )

        #expect(message == "Conflicts with the Quick Terminal shortcut.")
    }

    @Test("command prefix reset reports a Quick Terminal default conflict")
    func commandPrefixResetConflict() throws {
        let combo = CommandShortcutConfiguration().prefixCombo
        let virtualKeyCode = try #require(KeyCombo.virtualKeyCode(for: combo.key))
        let shortcut = QuickTerminalShortcut.keyCombo(
            combo,
            virtualKeyCode: virtualKeyCode
        )

        let message = QuickTerminalShortcutConflictResolver.commandPrefixResetConflictMessage(
            shortcut: shortcut
        )

        #expect(message == "Conflicts with the Quick Terminal shortcut.")
    }
}
