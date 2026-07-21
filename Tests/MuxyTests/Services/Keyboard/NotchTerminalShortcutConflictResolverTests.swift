import Testing

@testable import Muxy

@Suite("NotchTerminalShortcutConflictResolver")
@MainActor
struct NotchTerminalShortcutConflictResolverTests {
    @Test("app shortcut reset reports a Notch Terminal default conflict")
    func appShortcutResetConflict() throws {
        let binding = try #require(KeyBinding.defaults.first)
        let shortcut = NotchTerminalShortcut.keyCombo(binding.combo, virtualKeyCode: 0)

        let message = NotchTerminalShortcutConflictResolver.appShortcutResetConflictMessage(
            for: binding.action,
            shortcut: shortcut
        )

        #expect(message == "Conflicts with the Notch Terminal shortcut.")
    }

    @Test("command prefix reset reports a Notch Terminal default conflict")
    func commandPrefixResetConflict() {
        let shortcut = NotchTerminalShortcut.keyCombo(
            CommandShortcutConfiguration().prefixCombo,
            virtualKeyCode: 0
        )

        let message = NotchTerminalShortcutConflictResolver.commandPrefixResetConflictMessage(
            shortcut: shortcut
        )

        #expect(message == "Conflicts with the Notch Terminal shortcut.")
    }
}
