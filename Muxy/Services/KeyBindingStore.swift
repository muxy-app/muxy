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
            ?? KeyBinding.defaults.first { $0.action == action }
            ?? KeyBinding(action: action, combo: KeyCombo(key: "", modifiers: 0))
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

    func isRegisteredShortcut(event: NSEvent, scopes: Set<ShortcutScope>) -> Bool {
        action(for: event, scopes: scopes) != nil
    }

    func action(for event: NSEvent, scopes: Set<ShortcutScope>) -> ShortcutAction? {
        let normalizedKey = KeyCombo.normalized(
            key: event.charactersIgnoringModifiers ?? "",
            keyCode: event.keyCode
        )
        let flags = event.modifierFlags.intersection(KeyCombo.supportedModifierMask).rawValue
        return ShortcutAction.allCases.first { action in
            guard scopes.contains(action.scope) else { return false }
            let combo = combo(for: action)
            return combo.key == normalizedKey && combo.modifiers == flags
        }
    }

    func conflictingAction(for combo: KeyCombo, excluding: ShortcutAction) -> ShortcutAction? {
        conflictingAction(for: combo, excluding: Optional(excluding))
    }

    func conflictingAction(for combo: KeyCombo, excluding: ShortcutAction?) -> ShortcutAction? {
        bindings.first { binding in
            if let excluding {
                return binding.combo == combo && binding.action != excluding
            }
            return binding.combo == combo
        }?.action
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
