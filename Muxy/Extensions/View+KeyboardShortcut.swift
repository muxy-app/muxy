import SwiftUI

extension View {
    @ViewBuilder
    func shortcut(for action: ShortcutAction, store: KeyBindingStore) -> some View {
        let combo = store.combo(for: action)
        if combo.isAssigned {
            keyboardShortcut(combo.swiftUIKeyEquivalent, modifiers: combo.swiftUIModifiers)
        } else {
            self
        }
    }
}
