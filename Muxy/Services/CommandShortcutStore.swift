import AppKit
import os

private let commandShortcutLogger = Logger(subsystem: "app.muxy", category: "CommandShortcutStore")

protocol CommandShortcutPersisting {
    func loadConfiguration() throws -> CommandShortcutConfiguration
    func saveConfiguration(_ configuration: CommandShortcutConfiguration) throws
}

final class FileCommandShortcutPersistence: CommandShortcutPersisting {
    private let store: CodableFileStore<StoredCommandShortcutConfiguration>

    init(fileURL: URL = MuxyFileStorage.fileURL(filename: "command-shortcuts.json")) {
        store = CodableFileStore(
            fileURL: fileURL,
            options: CodableFileStoreOptions(
                prettyPrinted: true,
                sortedKeys: true,
                filePermissions: FilePermissions.privateFile
            )
        )
    }

    func loadConfiguration() throws -> CommandShortcutConfiguration {
        try store.load()?.configuration ?? CommandShortcutConfiguration()
    }

    func saveConfiguration(_ configuration: CommandShortcutConfiguration) throws {
        try store.save(StoredCommandShortcutConfiguration(configuration: configuration))
    }
}

struct CommandShortcutConfiguration: Equatable {
    var prefixCombo: KeyCombo
    var shortcuts: [CommandShortcut]

    init(
        prefixCombo: KeyCombo = KeyCombo(key: "g", command: true),
        shortcuts: [CommandShortcut] = []
    ) {
        self.prefixCombo = prefixCombo
        self.shortcuts = shortcuts
    }
}

private struct StoredCommandShortcutConfiguration: Codable {
    private enum CodingKeys: String, CodingKey {
        case prefixCombo
        case shortcuts
    }

    let prefixCombo: KeyCombo
    let shortcuts: [CommandShortcut]

    var configuration: CommandShortcutConfiguration {
        CommandShortcutConfiguration(prefixCombo: prefixCombo, shortcuts: shortcuts)
    }

    init(configuration: CommandShortcutConfiguration) {
        prefixCombo = configuration.prefixCombo
        shortcuts = configuration.shortcuts
    }

    init(from decoder: Decoder) throws {
        if let legacyShortcuts = try? [CommandShortcut](from: decoder) {
            prefixCombo = CommandShortcutConfiguration().prefixCombo
            shortcuts = legacyShortcuts
            return
        }
        let container = try decoder.container(keyedBy: CodingKeys.self)
        prefixCombo = try container.decodeIfPresent(KeyCombo.self, forKey: .prefixCombo)
            ?? CommandShortcutConfiguration().prefixCombo
        shortcuts = try container.decodeIfPresent([CommandShortcut].self, forKey: .shortcuts) ?? []
    }
}

@MainActor
@Observable
final class CommandShortcutStore {
    static let shared = CommandShortcutStore()

    private(set) var prefixCombo = CommandShortcutConfiguration().prefixCombo
    private(set) var shortcuts: [CommandShortcut] = []
    private(set) var isLayerActive = false
    private let persistence: any CommandShortcutPersisting
    @ObservationIgnored private var layerResetTask: Task<Void, Never>?
    private static let layerTimeout: Duration = .seconds(2)

    init(persistence: any CommandShortcutPersisting = FileCommandShortcutPersistence()) {
        self.persistence = persistence
        load()
    }

    func addShortcut() -> CommandShortcut {
        let shortcut = CommandShortcut()
        shortcuts.append(shortcut)
        save()
        return shortcut
    }

    func updateShortcut(_ shortcut: CommandShortcut) {
        guard let index = shortcuts.firstIndex(where: { $0.id == shortcut.id }) else { return }
        shortcuts[index] = shortcutWithDefaultModifier(shortcut)
        save()
    }

    func deleteShortcut(id: UUID) {
        shortcuts.removeAll { $0.id == id }
        save()
    }

    func deleteAllShortcuts() {
        guard !shortcuts.isEmpty else { return }
        shortcuts = []
        save()
    }

    func updatePrefixCombo(_ combo: KeyCombo) {
        prefixCombo = combo
        save()
    }

    func resetPrefixCombo() {
        updatePrefixCombo(CommandShortcutConfiguration().prefixCombo)
    }

    func activateLayer() {
        isLayerActive = true
        layerResetTask?.cancel()
        layerResetTask = Task { @MainActor [weak self] in
            try? await Task.sleep(for: Self.layerTimeout)
            self?.deactivateLayer()
        }
    }

    func deactivateLayer() {
        isLayerActive = false
        layerResetTask?.cancel()
        layerResetTask = nil
    }

    func matchesPrefix(event: NSEvent, scopes: Set<ShortcutScope>) -> Bool {
        guard scopes.contains(.mainWindow) else { return false }
        let normalizedKey = KeyCombo.normalized(
            key: event.charactersIgnoringModifiers ?? "",
            keyCode: event.keyCode
        )
        let flags = event.modifierFlags.intersection(KeyCombo.supportedModifierMask).rawValue
        return prefixCombo.key == normalizedKey && prefixCombo.modifiers == flags
    }

    func shortcut(for event: NSEvent, scopes: Set<ShortcutScope>) -> CommandShortcut? {
        guard scopes.contains(.mainWindow), isLayerActive else { return nil }
        let normalizedKey = KeyCombo.normalized(
            key: event.charactersIgnoringModifiers ?? "",
            keyCode: event.keyCode
        )
        let flags = event.modifierFlags.intersection(KeyCombo.supportedModifierMask).rawValue
        return shortcuts.first { shortcut in
            !shortcut.trimmedCommand.isEmpty
                && shortcut.combo.key == normalizedKey
                && shortcut.combo.modifiers == flags
        }
    }

    func isRegisteredShortcut(event: NSEvent, scopes: Set<ShortcutScope>) -> Bool {
        matchesPrefix(event: event, scopes: scopes) || shortcut(for: event, scopes: scopes) != nil
    }

    func conflictingShortcut(for combo: KeyCombo, excluding id: UUID) -> CommandShortcut? {
        let normalizedCombo = comboWithDefaultModifier(combo)
        return shortcuts.first { $0.combo == normalizedCombo && $0.id != id }
    }

    private func shortcutWithDefaultModifier(_ shortcut: CommandShortcut) -> CommandShortcut {
        var shortcut = shortcut
        shortcut.combo = comboWithDefaultModifier(shortcut.combo)
        return shortcut
    }

    private func comboWithDefaultModifier(_ combo: KeyCombo) -> KeyCombo {
        guard combo.nsModifierFlags.isEmpty else { return combo }
        return KeyCombo(key: combo.key, command: true)
    }

    private func load() {
        do {
            let configuration = try persistence.loadConfiguration()
            prefixCombo = configuration.prefixCombo
            shortcuts = configuration.shortcuts.map(shortcutWithDefaultModifier)
        } catch {
            commandShortcutLogger.error("Failed to load command shortcuts: \(error.localizedDescription)")
            prefixCombo = CommandShortcutConfiguration().prefixCombo
            shortcuts = []
        }
    }

    private func save() {
        do {
            try persistence.saveConfiguration(CommandShortcutConfiguration(
                prefixCombo: prefixCombo,
                shortcuts: shortcuts
            ))
        } catch {
            commandShortcutLogger.error("Failed to save command shortcuts: \(error.localizedDescription)")
        }
    }
}
