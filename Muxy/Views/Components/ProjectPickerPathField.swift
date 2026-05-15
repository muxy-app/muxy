import AppKit
import Carbon.HIToolbox
import SwiftUI

struct ProjectPickerPathField: NSViewRepresentable {
    @Binding var text: String
    let onCommand: (ProjectPickerCommand) -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(parent: self)
    }

    func makeNSView(context: Context) -> NSTextField {
        let field = ProjectPickerNSTextField()
        field.delegate = context.coordinator
        field.isBordered = false
        field.drawsBackground = false
        field.focusRingType = .none
        field.font = .monospacedSystemFont(ofSize: UIMetrics.fontEmphasis, weight: .regular)
        field.textColor = NSColor(MuxyTheme.fg)
        field.stringValue = text
        field.onCommand = onCommand
        DispatchQueue.main.async {
            field.window?.makeFirstResponder(field)
            field.moveCursorToEnd()
        }
        return field
    }

    func updateNSView(_ nsView: NSTextField, context: Context) {
        context.coordinator.parent = self
        if nsView.stringValue != text {
            nsView.stringValue = text
        }
        if let field = nsView as? ProjectPickerNSTextField {
            field.onCommand = onCommand
        }
    }

    @MainActor
    final class Coordinator: NSObject, NSTextFieldDelegate {
        var parent: ProjectPickerPathField

        init(parent: ProjectPickerPathField) {
            self.parent = parent
        }

        func controlTextDidChange(_ obj: Notification) {
            guard let field = obj.object as? NSTextField else { return }
            parent.text = field.stringValue
        }

        func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
            if commandSelector == #selector(NSResponder.insertNewline(_:)) {
                parent.onCommand(.openHighlighted)
                return true
            }
            if commandSelector == #selector(NSResponder.insertTab(_:)) {
                parent.onCommand(.completeHighlighted)
                return true
            }
            if commandSelector == #selector(NSResponder.moveUp(_:)) {
                parent.onCommand(.moveHighlightUp)
                return true
            }
            if commandSelector == #selector(NSResponder.moveDown(_:)) {
                parent.onCommand(.moveHighlightDown)
                return true
            }
            if commandSelector == #selector(NSResponder.deleteWordBackward(_:)), shouldGoUpOnDeleteBackward(textView) {
                parent.onCommand(.goBack)
                return true
            }
            if commandSelector == #selector(NSResponder.deleteBackward(_:)), shouldGoUpOnDeleteBackward(textView) {
                parent.onCommand(.goBack)
                return true
            }
            return false
        }

        private func shouldGoUpOnDeleteBackward(_ textView: NSTextView) -> Bool {
            let selectedRange = textView.selectedRange()
            guard selectedRange.length == 0, selectedRange.location == textView.string.utf16.count else { return false }
            let value = textView.string.trimmingCharacters(in: .whitespacesAndNewlines)
            return value.isEmpty || value == "~" || value.hasSuffix("/")
        }
    }
}

private final class ProjectPickerNSTextField: NSTextField {
    var onCommand: ((ProjectPickerCommand) -> Void)?

    func moveCursorToEnd() {
        guard let editor = currentEditor() else { return }
        editor.selectedRange = NSRange(location: stringValue.utf16.count, length: 0)
    }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        guard window?.firstResponder === currentEditor() else {
            return super.performKeyEquivalent(with: event)
        }
        if event.keyCode == kVK_Escape {
            onCommand?(.dismiss)
            return true
        }
        if event.keyCode == kVK_Return, event.modifierFlags.contains(.command) {
            onCommand?(.confirmTypedPath)
            return true
        }
        return super.performKeyEquivalent(with: event)
    }
}
