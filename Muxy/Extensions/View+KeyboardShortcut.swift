import SwiftUI

extension View {
    func shortcut(for action: ShortcutAction, store: KeyBindingStore) -> some View {
        let combo = store.combo(for: action)
        return keyboardShortcut(combo.swiftUIKeyEquivalent, modifiers: combo.swiftUIModifiers)
    }
}
