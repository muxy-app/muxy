import AppKit
import Carbon.HIToolbox
import Foundation

@MainActor
enum TranscriptInserter {
    static func insert(
        text: String,
        into responder: NSResponder?,
        appendReturn: Bool
    ) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }

        guard let responder, let window = window(for: responder) else {
            copyToClipboardWithToast(trimmed)
            return
        }

        window.makeKey()
        NSApp.activate(ignoringOtherApps: true)
        window.makeFirstResponder(responder)

        if let terminal = nearestGhosttyView(from: responder) {
            sendToTerminal(text: trimmed, view: terminal, appendReturn: appendReturn)
            return
        }

        if let textView = responder as? NSTextView {
            textView.insertText(trimmed, replacementRange: textView.selectedRange())
            if appendReturn {
                postReturnKey()
            }
            return
        }

        if let control = responder as? NSTextField {
            insert(text: trimmed, into: control, appendReturn: appendReturn)
            return
        }

        if let textInput = responder as? NSTextInputClient {
            textInput.insertText(trimmed, replacementRange: NSRange(location: NSNotFound, length: 0))
            if appendReturn {
                postReturnKey()
            }
            return
        }

        copyToClipboardWithToast(trimmed)
    }

    private static func window(for responder: NSResponder) -> NSWindow? {
        if let view = responder as? NSView { return view.window }
        if let viewController = responder as? NSViewController { return viewController.view.window }
        if let window = responder as? NSWindow { return window }
        return NSApp.keyWindow
    }

    private static func nearestGhosttyView(from responder: NSResponder) -> GhosttyTerminalNSView? {
        var current: NSResponder? = responder
        while let node = current {
            if let view = node as? GhosttyTerminalNSView { return view }
            current = node.nextResponder
        }
        if let view = responder as? NSView {
            var ancestor: NSView? = view
            while let node = ancestor {
                if let view = node as? GhosttyTerminalNSView { return view }
                ancestor = node.superview
            }
        }
        return nil
    }

    private static func sendToTerminal(text: String, view: GhosttyTerminalNSView, appendReturn: Bool) {
        let sanitized = text.replacingOccurrences(of: "\u{1B}[201~", with: "")
        var payload = Data()
        payload.append(TerminalControlBytes.bracketedPasteStart)
        payload.append(Data(sanitized.utf8))
        payload.append(TerminalControlBytes.bracketedPasteEnd)
        if appendReturn {
            payload.append(TerminalControlBytes.carriageReturn)
        }
        view.sendRemoteBytes(payload)
        view.window?.makeFirstResponder(view)
    }

    private static func insert(text: String, into textField: NSTextField, appendReturn: Bool) {
        if let editor = textField.currentEditor() as? NSTextView {
            editor.insertText(text, replacementRange: editor.selectedRange())
        } else {
            let current = textField.stringValue
            textField.stringValue = current + text
        }
        if appendReturn {
            postReturnKey()
        }
    }

    private static func postReturnKey() {
        guard let source = CGEventSource(stateID: .combinedSessionState) else { return }
        if let down = CGEvent(keyboardEventSource: source, virtualKey: CGKeyCode(kVK_Return), keyDown: true) {
            down.post(tap: .cghidEventTap)
        }
        if let up = CGEvent(keyboardEventSource: source, virtualKey: CGKeyCode(kVK_Return), keyDown: false) {
            up.post(tap: .cghidEventTap)
        }
    }

    private static func copyToClipboardWithToast(_ text: String) {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(text, forType: .string)
        ToastState.shared.show("Transcript copied to clipboard")
    }
}
