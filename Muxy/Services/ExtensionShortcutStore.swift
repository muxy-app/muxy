import AppKit
import os

private let extensionShortcutLogger = Logger(subsystem: "app.muxy", category: "ExtensionShortcutStore")

protocol ExtensionShortcutPersisting {
    func loadShortcuts() throws -> [ExtensionShortcut]
    func saveShortcuts(_ shortcuts: [ExtensionShortcut]) throws
}

final class FileExtensionShortcutPersistence: ExtensionShortcutPersisting {
    private let store: CodableFileStore<[ExtensionShortcut]>

    init(fileURL: URL = MuxyFileStorage.fileURL(filename: "extension-shortcuts.json")) {
        store = CodableFileStore(
            fileURL: fileURL,
            options: CodableFileStoreOptions(
                prettyPrinted: true,
                sortedKeys: true,
                filePermissions: FilePermissions.privateFile
            )
        )
    }

    func loadShortcuts() throws -> [ExtensionShortcut] {
        try store.load() ?? []
    }

    func saveShortcuts(_ shortcuts: [ExtensionShortcut]) throws {
        try store.save(shortcuts)
    }
}

@MainActor
@Observable
final class ExtensionShortcutStore {
    static let shared = ExtensionShortcutStore()

    private(set) var shortcuts: [ExtensionShortcut] = []
    private(set) var runtimeShortcuts: [ExtensionShortcut] = []
    private let persistence: any ExtensionShortcutPersisting

    private var allShortcuts: [ExtensionShortcut] { shortcuts + runtimeShortcuts }

    init(persistence: any ExtensionShortcutPersisting = FileExtensionShortcutPersistence()) {
        self.persistence = persistence
        load()
    }

    func shortcut(extensionID: String, commandID: String) -> ExtensionShortcut? {
        shortcuts.first { $0.extensionID == extensionID && $0.commandID == commandID }
    }

    func shortcuts(forExtension extensionID: String) -> [ExtensionShortcut] {
        allShortcuts.filter { $0.extensionID == extensionID }
    }

    @discardableResult
    func register(extensionID: String, commandID: String, combo comboString: String) throws -> String? {
        guard !commandID.isEmpty else {
            throw APIError.invalidArguments("shortcut id must not be empty")
        }
        guard let combo = KeyCombo(parsing: comboString) else {
            throw APIError.invalidArguments("invalid shortcut '\(comboString)'")
        }
        if let conflict = conflictMessage(for: combo, extensionID: extensionID, commandID: commandID) {
            return conflict
        }
        let shortcut = ExtensionShortcut(extensionID: extensionID, commandID: commandID, combo: combo, source: .runtime)
        if let index = runtimeShortcuts.firstIndex(where: {
            $0.extensionID == extensionID && $0.commandID == commandID
        }) {
            runtimeShortcuts[index] = shortcut
        } else {
            runtimeShortcuts.append(shortcut)
        }
        return nil
    }

    func unregister(extensionID: String, commandID: String) {
        runtimeShortcuts.removeAll { $0.extensionID == extensionID && $0.commandID == commandID }
    }

    func clearRuntimeShortcuts(keepingExtensionIDs keep: Set<String>) {
        runtimeShortcuts.removeAll { !keep.contains($0.extensionID) }
    }

    func updateCombo(extensionID: String, commandID: String, combo: KeyCombo) {
        guard let index = shortcuts.firstIndex(where: {
            $0.extensionID == extensionID && $0.commandID == commandID
        })
        else { return }
        shortcuts[index].combo = combo
        save()
    }

    func resetCombo(extensionID: String, commandID: String, defaultCombo: KeyCombo?) {
        let combo = defaultCombo.flatMap { isComboFree($0, extensionID: extensionID, commandID: commandID) ? $0 : nil }
        updateCombo(extensionID: extensionID, commandID: commandID, combo: combo ?? KeyCombo(key: "", modifiers: 0))
    }

    func unassign(extensionID: String, commandID: String) {
        updateCombo(extensionID: extensionID, commandID: commandID, combo: KeyCombo(key: "", modifiers: 0))
    }

    func match(event: NSEvent, scopes: Set<ShortcutScope>) -> ExtensionShortcut? {
        guard scopes.contains(.mainWindow) else { return nil }
        let normalizedKey = KeyCombo.normalized(
            key: event.charactersIgnoringModifiers ?? "",
            keyCode: event.keyCode
        )
        let flags = event.modifierFlags.intersection(KeyCombo.supportedModifierMask).rawValue
        let isMatch: (ExtensionShortcut) -> Bool = { shortcut in
            shortcut.combo.isAssigned
                && shortcut.combo.key == normalizedKey
                && shortcut.combo.modifiers == flags
        }
        return shortcuts.first(where: isMatch) ?? runtimeShortcuts.first(where: isMatch)
    }

    func isRegisteredShortcut(event: NSEvent, scopes: Set<ShortcutScope>) -> Bool {
        match(event: event, scopes: scopes) != nil
    }

    func conflictMessage(for combo: KeyCombo, extensionID: String, commandID: String) -> String? {
        guard combo.isAssigned else { return nil }
        if let action = KeyBindingStore.shared.conflictingAction(for: combo, excluding: ShortcutAction?.none) {
            return "Conflicts with \"\(action.displayName)\""
        }
        if CommandShortcutStore.shared.conflictingShortcut(for: combo, excluding: UUID()) != nil {
            return "Conflicts with a custom command"
        }
        let conflictsWithOther = allShortcuts.contains {
            $0.combo == combo && !($0.extensionID == extensionID && $0.commandID == commandID)
        }
        return conflictsWithOther ? "Conflicts with another extension shortcut" : nil
    }

    func syncBindings(for extensions: [MuxyExtension]) {
        var resolved: [ExtensionShortcut] = []
        let stored = Dictionary(uniqueKeysWithValues: shortcuts.map { ($0.id, $0) })

        for muxyExtension in extensions {
            for command in muxyExtension.manifest.commands where command.defaultShortcut != nil {
                let key = "\(muxyExtension.id):\(command.id)"
                if let existing = stored[key] {
                    resolved.append(existing)
                    continue
                }
                let combo = autoAssignCombo(
                    command.defaultCombo,
                    extensionID: muxyExtension.id,
                    commandID: command.id,
                    claimed: resolved
                )
                resolved.append(ExtensionShortcut(
                    extensionID: muxyExtension.id,
                    commandID: command.id,
                    combo: combo
                ))
            }
        }

        guard resolved != shortcuts else { return }
        shortcuts = resolved
        save()
    }

    private func autoAssignCombo(
        _ defaultCombo: KeyCombo?,
        extensionID: String,
        commandID: String,
        claimed: [ExtensionShortcut]
    ) -> KeyCombo {
        guard let defaultCombo else { return KeyCombo(key: "", modifiers: 0) }
        let conflictsWithClaimed = claimed.contains { $0.combo == defaultCombo }
        guard !conflictsWithClaimed,
              isComboFree(defaultCombo, extensionID: extensionID, commandID: commandID)
        else { return KeyCombo(key: "", modifiers: 0) }
        return defaultCombo
    }

    private func isComboFree(_ combo: KeyCombo, extensionID: String, commandID: String) -> Bool {
        guard combo.isAssigned else { return false }
        guard KeyBindingStore.shared.conflictingAction(for: combo, excluding: ShortcutAction?.none) == nil else {
            return false
        }
        guard CommandShortcutStore.shared.conflictingShortcut(for: combo, excluding: UUID()) == nil else {
            return false
        }
        return !allShortcuts.contains {
            $0.combo == combo && !($0.extensionID == extensionID && $0.commandID == commandID)
        }
    }

    private func load() {
        do {
            shortcuts = try persistence.loadShortcuts()
        } catch {
            extensionShortcutLogger.error("Failed to load extension shortcuts: \(error.localizedDescription)")
            shortcuts = []
        }
    }

    private func save() {
        do {
            try persistence.saveShortcuts(shortcuts)
        } catch {
            extensionShortcutLogger.error("Failed to save extension shortcuts: \(error.localizedDescription)")
        }
    }
}
