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
            if let command = ProjectPickerPathFieldCommandMapper.command(
                for: commandSelector,
                shouldGoUpOnDeleteBackward: shouldGoUpOnDeleteBackward(textView)
            ) {
                parent.onCommand(command)
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
        if let command = ProjectPickerPathFieldCommandMapper.command(for: event) {
            onCommand?(command)
            return true
        }
        return super.performKeyEquivalent(with: event)
    }
}

private enum ProjectPickerPathFieldCommandMapper {
    static func command(for selector: Selector, shouldGoUpOnDeleteBackward: Bool) -> ProjectPickerCommand? {
        if selector == #selector(NSResponder.insertNewline(_:)) { return .openHighlighted }
        if selector == #selector(NSResponder.insertTab(_:)) { return .completeHighlighted }
        if selector == #selector(NSResponder.moveUp(_:)) { return .moveHighlightUp }
        if selector == #selector(NSResponder.moveDown(_:)) { return .moveHighlightDown }
        if selector == #selector(NSResponder.deleteWordBackward(_:)) { return shouldGoUpOnDeleteBackward ? .goBack : nil }
        if selector == #selector(NSResponder.deleteBackward(_:)) { return shouldGoUpOnDeleteBackward ? .goBack : nil }
        return nil
    }

    static func command(for event: NSEvent) -> ProjectPickerCommand? {
        if event.keyCode == kVK_Escape { return .dismiss }
        if event.keyCode == kVK_Return, event.modifierFlags.contains(.command) { return .confirmTypedPath }
        return nil
    }
}
