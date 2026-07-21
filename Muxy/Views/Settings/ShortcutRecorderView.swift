import AppKit
import SwiftUI

struct ShortcutRecorderView: NSViewRepresentable {
    let onRecord: (KeyCombo) -> Bool
    let onCancel: () -> Void
    var requiresModifier = true
    var onRecordWithKeyCode: ((KeyCombo, UInt16) -> Bool)?

    func makeNSView(context: Context) -> ShortcutRecorderNSView {
        let view = ShortcutRecorderNSView()
        view.onRecord = onRecord
        view.onCancel = onCancel
        view.requiresModifier = requiresModifier
        view.onRecordWithKeyCode = onRecordWithKeyCode
        DispatchQueue.main.async { view.window?.makeFirstResponder(view) }
        return view
    }

    func updateNSView(_ nsView: ShortcutRecorderNSView, context: Context) {
        nsView.onRecord = onRecord
        nsView.onCancel = onCancel
        nsView.requiresModifier = requiresModifier
        nsView.onRecordWithKeyCode = onRecordWithKeyCode
    }
}

final class ShortcutRecorderNSView: NSView {
    var onRecord: ((KeyCombo) -> Bool)?
    var onRecordWithKeyCode: ((KeyCombo, UInt16) -> Bool)?
    var onCancel: (() -> Void)?
    var requiresModifier = true
    private var completed = false

    override var acceptsFirstResponder: Bool { true }

    override func becomeFirstResponder() -> Bool {
        completed = false
        return super.becomeFirstResponder()
    }

    override func resignFirstResponder() -> Bool {
        let result = super.resignFirstResponder()
        if result, !completed {
            onCancel?()
        }
        return result
    }

    override func accessibilityRole() -> NSAccessibility.Role? {
        .textField
    }

    override func accessibilityRoleDescription() -> String? {
        "Shortcut Recorder"
    }

    override func accessibilityLabel() -> String? {
        "Press a keyboard shortcut to assign, or Escape to cancel"
    }

    override func accessibilityValue() -> Any? {
        "Recording"
    }

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
            completed = true
            onCancel?()
            return true
        }

        let flags = event.modifierFlags.intersection(KeyCombo.supportedModifierMask)
        let hasModifier = flags.contains(.command) || flags.contains(.control) || flags.contains(.option)
        guard hasModifier || !requiresModifier else { return false }

        let key = KeyCombo.normalized(key: event.charactersIgnoringModifiers ?? "", keyCode: event.keyCode)
        guard !key.isEmpty else { return false }

        let combo = KeyCombo(key: key, modifiers: flags.rawValue)
        if let onRecordWithKeyCode {
            completed = onRecordWithKeyCode(combo, event.keyCode)
        } else {
            completed = onRecord?(combo) ?? false
        }
        return true
    }
}
