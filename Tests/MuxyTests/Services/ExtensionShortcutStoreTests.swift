import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionShortcutStore")
@MainActor
struct ExtensionShortcutStoreTests {
    @Test("syncBindings auto-assigns a free default combo")
    func autoAssignsFreeCombo() {
        let persistence = InMemoryExtensionShortcutPersistence()
        let store = ExtensionShortcutStore(persistence: persistence)
        let ext = makeExtension(id: "alpha", commandID: "open", shortcut: "ctrl+opt+shift+1")

        store.syncBindings(for: [ext])

        let shortcut = store.shortcut(extensionID: "alpha", commandID: "open")
        #expect(shortcut?.combo == KeyCombo(key: "1", shift: true, control: true, option: true))
        #expect(persistence.savedShortcuts?.count == 1)
    }

    @Test("syncBindings registers later extension unassigned when combo already claimed")
    func registersUnassignedOnConflict() {
        let store = ExtensionShortcutStore(persistence: InMemoryExtensionShortcutPersistence())
        let first = makeExtension(id: "alpha", commandID: "open", shortcut: "ctrl+opt+shift+2")
        let second = makeExtension(id: "beta", commandID: "open", shortcut: "ctrl+opt+shift+2")

        store.syncBindings(for: [first, second])

        #expect(store.shortcut(extensionID: "alpha", commandID: "open")?.combo.isAssigned == true)
        #expect(store.shortcut(extensionID: "beta", commandID: "open")?.combo.isAssigned == false)
    }

    @Test("syncBindings preserves stored user-assigned combos")
    func preservesStoredCombo() {
        let stored = ExtensionShortcut(
            extensionID: "alpha",
            commandID: "open",
            combo: KeyCombo(key: "9", command: true)
        )
        let persistence = InMemoryExtensionShortcutPersistence(shortcuts: [stored])
        let store = ExtensionShortcutStore(persistence: persistence)
        let ext = makeExtension(id: "alpha", commandID: "open", shortcut: "ctrl+opt+shift+3")

        store.syncBindings(for: [ext])

        #expect(store.shortcut(extensionID: "alpha", commandID: "open")?.combo == KeyCombo(key: "9", command: true))
    }

    @Test("syncBindings drops shortcuts for commands no longer present")
    func dropsRemovedCommands() {
        let stored = ExtensionShortcut(
            extensionID: "alpha",
            commandID: "gone",
            combo: KeyCombo(key: "9", command: true)
        )
        let store = ExtensionShortcutStore(persistence: InMemoryExtensionShortcutPersistence(shortcuts: [stored]))

        store.syncBindings(for: [])

        #expect(store.shortcuts.isEmpty)
    }

    @Test("unassign clears the combo")
    func unassignClearsCombo() {
        let stored = ExtensionShortcut(
            extensionID: "alpha",
            commandID: "open",
            combo: KeyCombo(key: "9", command: true)
        )
        let persistence = InMemoryExtensionShortcutPersistence(shortcuts: [stored])
        let store = ExtensionShortcutStore(persistence: persistence)

        store.unassign(extensionID: "alpha", commandID: "open")

        #expect(store.shortcut(extensionID: "alpha", commandID: "open")?.combo.isAssigned == false)
        #expect(persistence.savedShortcuts?.first?.combo.isAssigned == false)
    }

    private func makeExtension(id: String, commandID: String, shortcut: String) -> MuxyExtension {
        let command = ExtensionPaletteCommand(id: commandID, title: commandID, defaultShortcut: shortcut)
        let manifest = ExtensionManifest(name: id, version: "1.0.0", commands: [command])
        return MuxyExtension(id: id, directory: URL(fileURLWithPath: "/tmp/\(id)"), manifest: manifest)
    }
}

@Suite("KeyCombo parsing")
struct KeyComboParsingTests {
    @Test("parses modifiers and key from string")
    func parsesCombo() {
        #expect(KeyCombo(parsing: "cmd+shift+e") == KeyCombo(key: "e", command: true, shift: true))
        #expect(KeyCombo(parsing: "ctrl+opt+k") == KeyCombo(key: "k", command: false, control: true, option: true))
        #expect(KeyCombo(parsing: "cmd+return") == KeyCombo(key: KeyCombo.returnKey, command: true))
    }

    @Test("rejects combos without a command, control, or option modifier")
    func rejectsModifierlessCombos() {
        #expect(KeyCombo(parsing: "e") == nil)
        #expect(KeyCombo(parsing: "shift+e") == nil)
        #expect(KeyCombo(parsing: "return") == nil)
    }

    @Test("returns nil for unparseable strings")
    func rejectsInvalid() {
        #expect(KeyCombo(parsing: "") == nil)
        #expect(KeyCombo(parsing: "cmd+bogus") == nil)
        #expect(KeyCombo(parsing: "cmd+") == nil)
    }
}

private final class InMemoryExtensionShortcutPersistence: ExtensionShortcutPersisting {
    var shortcuts: [ExtensionShortcut]
    var savedShortcuts: [ExtensionShortcut]?

    init(shortcuts: [ExtensionShortcut] = []) {
        self.shortcuts = shortcuts
    }

    func loadShortcuts() throws -> [ExtensionShortcut] {
        shortcuts
    }

    func saveShortcuts(_ shortcuts: [ExtensionShortcut]) throws {
        savedShortcuts = shortcuts
    }
}
