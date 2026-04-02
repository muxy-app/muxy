import AppKit
import os

private let logger = Logger(subsystem: "app.muxy", category: "KeyBindingStore")

@MainActor
@Observable
final class KeyBindingStore {
    static let shared = KeyBindingStore()

    private(set) var bindings: [KeyBinding] = []
    private let persistence: any KeyBindingPersisting

    init(persistence: any KeyBindingPersisting = FileKeyBindingPersistence()) {
        self.persistence = persistence
        load()
    }

    func binding(for action: ShortcutAction) -> KeyBinding {
        bindings.first { $0.action == action }
            ?? KeyBinding.defaults.first { $0.action == action }!
    }

    func combo(for action: ShortcutAction) -> KeyCombo {
        binding(for: action).combo
    }

    func updateBinding(action: ShortcutAction, combo: KeyCombo) {
        guard let index = bindings.firstIndex(where: { $0.action == action }) else { return }
        bindings[index].combo = combo
        save()
    }

    func resetToDefaults() {
        bindings = KeyBinding.defaults
        save()
    }

    func resetBinding(action: ShortcutAction) {
        guard let defaultBinding = KeyBinding.defaults.first(where: { $0.action == action }) else { return }
        updateBinding(action: defaultBinding.action, combo: defaultBinding.combo)
    }

    func isRegisteredShortcut(
        key: String,
        modifiers: NSEvent.ModifierFlags,
        scopes: Set<ShortcutScope>
    ) -> Bool {
        let flags = modifiers.intersection(.deviceIndependentFlagsMask).rawValue
        return bindings.contains {
            $0.combo.key == key &&
            $0.combo.modifiers == flags &&
            scopes.contains($0.action.scope)
        }
    }

    func conflictingAction(for combo: KeyCombo, excluding: ShortcutAction) -> ShortcutAction? {
        bindings.first { $0.combo == combo && $0.action != excluding }?.action
    }

    private func load() {
        do {
            bindings = try persistence.loadBindings()
        } catch {
            logger.error("Failed to load key bindings: \(error.localizedDescription)")
            bindings = KeyBinding.defaults
        }
    }

    private func save() {
        do {
            try persistence.saveBindings(bindings)
        } catch {
            logger.error("Failed to save key bindings: \(error.localizedDescription)")
        }
    }
}
