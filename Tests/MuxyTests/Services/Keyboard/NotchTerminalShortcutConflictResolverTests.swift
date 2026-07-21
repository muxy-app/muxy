import Testing

@testable import Muxy

@Suite("NotchTerminalShortcutConflictResolver")
@MainActor
struct NotchTerminalShortcutConflictResolverTests {
    @Test("app shortcut reset reports a Notch Terminal default conflict")
    func appShortcutResetConflict() throws {
        let binding = try #require(KeyBinding.defaults.first)
        let virtualKeyCode = try #require(KeyCombo.virtualKeyCode(for: binding.combo.key))
        let shortcut = NotchTerminalShortcut.keyCombo(binding.combo, virtualKeyCode: virtualKeyCode)

        let message = NotchTerminalShortcutConflictResolver.appShortcutResetConflictMessage(
            for: binding.action,
            shortcut: shortcut
        )

        #expect(message == "Conflicts with the Notch Terminal shortcut.")
    }

    @Test("command prefix reset reports a Notch Terminal default conflict")
    func commandPrefixResetConflict() throws {
        let combo = CommandShortcutConfiguration().prefixCombo
        let virtualKeyCode = try #require(KeyCombo.virtualKeyCode(for: combo.key))
        let shortcut = NotchTerminalShortcut.keyCombo(
            combo,
            virtualKeyCode: virtualKeyCode
        )

        let message = NotchTerminalShortcutConflictResolver.commandPrefixResetConflictMessage(
            shortcut: shortcut
        )

        #expect(message == "Conflicts with the Notch Terminal shortcut.")
    }
}
