import Foundation
import Testing

@testable import Muxy

@Suite("CommandShortcutStore")
@MainActor
struct CommandShortcutStoreTests {
    @Test("default prefix combo is command g")
    func defaultPrefixCombo() {
        #expect(CommandShortcutConfiguration().prefixCombo == KeyCombo(key: "g", command: true))
    }

    @Test("addShortcut persists command shortcut")
    func addShortcut() {
        let persistence = InMemoryCommandShortcutPersistence()
        let store = CommandShortcutStore(persistence: persistence)

        let shortcut = store.addShortcut()

        #expect(store.shortcuts.count == 1)
        #expect(persistence.savedConfiguration?.shortcuts == [shortcut])
    }

    @Test("updateShortcut persists changes")
    func updateShortcut() {
        let shortcut = CommandShortcut(name: "Server", command: "npm run dev")
        let persistence = InMemoryCommandShortcutPersistence(shortcuts: [shortcut])
        let store = CommandShortcutStore(persistence: persistence)
        var updated = shortcut
        updated.name = "Tests"
        updated.command = "swift test"

        store.updateShortcut(updated)

        #expect(store.shortcuts == [updated])
        #expect(persistence.savedConfiguration?.shortcuts == [updated])
    }

    @Test("updateShortcut adds command modifier when combo has no modifiers")
    func updateShortcutAddsCommandModifier() {
        var shortcut = CommandShortcut(name: "Server", command: "npm run dev")
        shortcut.combo = KeyCombo(key: "s", modifiers: 0)
        let persistence = InMemoryCommandShortcutPersistence(shortcuts: [shortcut])
        let store = CommandShortcutStore(persistence: persistence)

        store.updateShortcut(shortcut)

        #expect(store.shortcuts.first?.combo == KeyCombo(key: "s", command: true))
        #expect(persistence.savedConfiguration?.shortcuts.first?.combo == KeyCombo(key: "s", command: true))
    }

    @Test("conflictingShortcut compares combos after default modifier is applied")
    func conflictingShortcutAppliesDefaultModifier() {
        let shortcut = CommandShortcut(name: "Server", command: "npm run dev", combo: KeyCombo(key: "s", command: true))
        let persistence = InMemoryCommandShortcutPersistence(shortcuts: [shortcut])
        let store = CommandShortcutStore(persistence: persistence)

        let conflict = store.conflictingShortcut(for: KeyCombo(key: "s", modifiers: 0), excluding: UUID())

        #expect(conflict == shortcut)
    }

    @Test("deleteShortcut removes shortcut")
    func deleteShortcut() {
        let shortcut = CommandShortcut(name: "Server", command: "npm run dev")
        let persistence = InMemoryCommandShortcutPersistence(shortcuts: [shortcut])
        let store = CommandShortcutStore(persistence: persistence)

        store.deleteShortcut(id: shortcut.id)

        #expect(store.shortcuts.isEmpty)
        #expect(persistence.savedConfiguration?.shortcuts.isEmpty == true)
    }

    @Test("deleteAllShortcuts removes all command shortcuts")
    func deleteAllShortcuts() {
        let prefix = KeyCombo(key: "j", command: true, shift: true)
        let persistence = InMemoryCommandShortcutPersistence(
            prefixCombo: prefix,
            shortcuts: [
                CommandShortcut(name: "Server", command: "npm run dev"),
                CommandShortcut(name: "Tests", command: "swift test"),
            ]
        )
        let store = CommandShortcutStore(persistence: persistence)

        store.deleteAllShortcuts()

        #expect(store.shortcuts.isEmpty)
        #expect(persistence.savedConfiguration == CommandShortcutConfiguration(prefixCombo: prefix))
    }

    @Test("updatePrefixCombo persists layer shortcut")
    func updatePrefixCombo() {
        let prefix = KeyCombo(key: "j", command: true, shift: true)
        let persistence = InMemoryCommandShortcutPersistence()
        let store = CommandShortcutStore(persistence: persistence)

        store.updatePrefixCombo(prefix)

        #expect(store.prefixCombo == prefix)
        #expect(persistence.savedConfiguration?.prefixCombo == prefix)
    }

    @Test("resetPrefixCombo restores default layer shortcut")
    func resetPrefixCombo() {
        let prefix = KeyCombo(key: "j", command: true, shift: true)
        let shortcut = CommandShortcut(name: "Server", command: "npm run dev")
        let persistence = InMemoryCommandShortcutPersistence(prefixCombo: prefix, shortcuts: [shortcut])
        let store = CommandShortcutStore(persistence: persistence)

        store.resetPrefixCombo()

        #expect(store.prefixCombo == CommandShortcutConfiguration().prefixCombo)
        #expect(persistence.savedConfiguration == CommandShortcutConfiguration(
            prefixCombo: CommandShortcutConfiguration().prefixCombo,
            shortcuts: [shortcut]
        ))
    }
}

private final class InMemoryCommandShortcutPersistence: CommandShortcutPersisting {
    var configuration: CommandShortcutConfiguration
    var savedConfiguration: CommandShortcutConfiguration?

    init(
        prefixCombo: KeyCombo = CommandShortcutConfiguration().prefixCombo,
        shortcuts: [CommandShortcut] = []
    ) {
        configuration = CommandShortcutConfiguration(prefixCombo: prefixCombo, shortcuts: shortcuts)
    }

    func loadConfiguration() throws -> CommandShortcutConfiguration {
        configuration
    }

    func saveConfiguration(_ configuration: CommandShortcutConfiguration) throws {
        savedConfiguration = configuration
    }
}
