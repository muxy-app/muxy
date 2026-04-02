import AppKit
import SwiftUI

struct ShortcutRecorderView: NSViewRepresentable {
    let onRecord: (KeyCombo) -> Void
    let onCancel: () -> Void

    func makeNSView(context: Context) -> ShortcutRecorderNSView {
        let view = ShortcutRecorderNSView()
        view.onRecord = onRecord
        view.onCancel = onCancel
        DispatchQueue.main.async { view.window?.makeFirstResponder(view) }
        return view
    }

    func updateNSView(_ nsView: ShortcutRecorderNSView, context: Context) {
        nsView.onRecord = onRecord
        nsView.onCancel = onCancel
    }
}

final class ShortcutRecorderNSView: NSView {
    var onRecord: ((KeyCombo) -> Void)?
    var onCancel: (() -> Void)?

    override var acceptsFirstResponder: Bool { true }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        guard event.type == .keyDown else { return super.performKeyEquivalent(with: event) }
        return handleKeyEvent(event)
    }

    override func keyDown(with event: NSEvent) {
        if !handleKeyEvent(event) {
            super.keyDown(with: event)
        }
    }

    @discardableResult
    private func handleKeyEvent(_ event: NSEvent) -> Bool {
        if event.keyCode == 53 {
            onCancel?()
            return true
        }

        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        let hasModifier = flags.contains(.command) || flags.contains(.control) || flags.contains(.option)
        guard hasModifier else { return false }

        let key = event.charactersIgnoringModifiers?.lowercased() ?? ""
        guard !key.isEmpty else { return false }

        onRecord?(KeyCombo(key: key, modifiers: flags.rawValue))
        return true
    }
}
