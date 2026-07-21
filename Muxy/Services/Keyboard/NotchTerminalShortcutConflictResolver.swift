import Foundation

@MainActor
enum NotchTerminalShortcutConflictResolver {
    static func conflictMessage(for combo: KeyCombo) -> String? {
        guard combo.isAssigned else { return nil }
        if let action = KeyBindingStore.shared.conflictingAction(for: combo, excluding: ShortcutAction?.none) {
            return "Conflicts with \"\(action.displayName)\"."
        }
        let commandStore = CommandShortcutStore.shared
        if commandStore.prefixCombo == combo {
            return "Conflicts with the command prefix."
        }
        if commandStore.conflictingShortcut(for: combo, excluding: UUID()) != nil {
            return "Conflicts with a custom command."
        }
        if ExtensionShortcutStore.shared.conflictingShortcut(
            for: combo,
            excludingExtensionID: nil,
            commandID: nil
        ) != nil {
            return "Conflicts with an extension shortcut."
        }
        return nil
    }

    static func notchTerminalConflictMessage(
        for combo: KeyCombo,
        shortcut: NotchTerminalShortcut? = nil
    ) -> String? {
        let shortcut = shortcut ?? NotchTerminalShortcutService.shared.shortcut
        guard shortcut.keyCombo == combo else { return nil }
        return "Conflicts with the Notch Terminal shortcut."
    }

    static func appShortcutResetConflictMessage(
        for action: ShortcutAction,
        shortcut: NotchTerminalShortcut? = nil
    ) -> String? {
        guard let combo = KeyBinding.defaults.first(where: { $0.action == action })?.combo else { return nil }
        return notchTerminalConflictMessage(for: combo, shortcut: shortcut)
    }

    static func commandPrefixResetConflictMessage(shortcut: NotchTerminalShortcut? = nil) -> String? {
        notchTerminalConflictMessage(
            for: CommandShortcutConfiguration().prefixCombo,
            shortcut: shortcut
        )
    }
}
