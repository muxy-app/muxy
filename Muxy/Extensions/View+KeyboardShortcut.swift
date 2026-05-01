import SwiftUI

extension View {
    func shortcut(for action: ShortcutAction, store: KeyBindingStore) -> some View {
        let combo = store.combo(for: action)
        if let keyEquivalent = combo.swiftUIKeyEquivalent {
            return AnyView(keyboardShortcut(keyEquivalent, modifiers: combo.swiftUIModifiers))
        }
        return AnyView(self)
    }
}
