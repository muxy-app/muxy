import AppKit
import SwiftUI
import Testing

@testable import Muxy

@MainActor
@Suite("PaletteSearchField")
struct PaletteSearchFieldTests {
    @Test("claims focus after late window attachment")
    func claimsFocusAfterLateWindowAttachment() async throws {
        let text = PaletteSearchFieldTextBox()
        let view = PaletteSearchField(
            text: Binding(
                get: { text.value },
                set: { text.value = $0 }
            ),
            placeholder: "Search",
            onSubmit: {},
            onEscape: {},
            onArrowUp: {},
            onArrowDown: {}
        )
        .frame(width: 240, height: 28)
        let hostingView = NSHostingView(rootView: view)
        hostingView.frame = NSRect(x: 0, y: 0, width: 240, height: 28)
        hostingView.layoutSubtreeIfNeeded()

        let field = try #require(textField(in: hostingView))
        #expect(field.window == nil)

        try await Task.sleep(for: .milliseconds(50))

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 320, height: 120),
            styleMask: [.titled],
            backing: .buffered,
            defer: false
        )
        window.contentView = hostingView
        window.makeKeyAndOrderFront(nil)

        try await waitForFocus(field)
        window.orderOut(nil)
    }

    @Test("invokes onEscape when field editor cancels")
    func invokesOnEscapeWhenFieldEditorCancels() {
        let escaped = PaletteSearchFieldFlag()
        let text = PaletteSearchFieldTextBox()
        let field = PaletteSearchField(
            text: Binding(
                get: { text.value },
                set: { text.value = $0 }
            ),
            placeholder: "Search",
            onSubmit: {},
            onEscape: { escaped.value = true },
            onArrowUp: {},
            onArrowDown: {}
        )
        let coordinator = field.makeCoordinator()
        let control = NSTextField()

        let handled = coordinator.control(
            control,
            textView: NSTextView(),
            doCommandBy: #selector(NSResponder.cancelOperation(_:))
        )

        #expect(handled)
        #expect(escaped.value)
    }

    @Test("submit exposes the latest control text")
    func submitExposesTheLatestControlText() {
        let text = PaletteSearchFieldTextBox()
        let submitted = PaletteSearchFieldTextBox()
        let field = PaletteSearchField(
            text: Binding(
                get: { text.value },
                set: { text.value = $0 }
            ),
            placeholder: "Search",
            onSubmit: {},
            onSubmitText: { submitted.value = $0 },
            onEscape: {},
            onArrowUp: {},
            onArrowDown: {}
        )
        let coordinator = field.makeCoordinator()
        let control = NSTextField()
        control.stringValue = "definitely-present"

        let handled = coordinator.control(
            control,
            textView: NSTextView(),
            doCommandBy: #selector(NSResponder.insertNewline(_:))
        )

        #expect(handled)
        #expect(text.value == "definitely-present")
        #expect(submitted.value == "definitely-present")
    }

    @Test("submits after IME return commits marked text")
    func submitsAfterIMEReturnCommitsMarkedText() {
        let text = PaletteSearchFieldTextBox()
        let submitted = PaletteSearchFieldTextBox()
        let field = PaletteSearchField(
            text: Binding(
                get: { text.value },
                set: { text.value = $0 }
            ),
            placeholder: "Search",
            onSubmit: {},
            onSubmitText: { submitted.value = $0 },
            onEscape: {},
            onArrowUp: {},
            onArrowDown: {}
        )
        let coordinator = field.makeCoordinator()
        let control = PaletteNSTextField()
        control.stringValue = "검색"
        control.deferSubmitAfterMarkedTextCommit()

        coordinator.controlTextDidChange(Notification(
            name: NSControl.textDidChangeNotification,
            object: control
        ))

        #expect(text.value == "검색")
        #expect(submitted.value == "검색")
    }

    @Test("emits query change after committed Korean text")
    func emitsQueryChangeAfterCommittedKoreanText() {
        let text = PaletteSearchFieldTextBox()
        let query = PaletteSearchFieldTextBox()
        let field = PaletteSearchField(
            text: Binding(
                get: { text.value },
                set: { text.value = $0 }
            ),
            placeholder: "Search",
            onSubmit: {},
            onEscape: {},
            onArrowUp: {},
            onArrowDown: {},
            onQueryChange: { query.value = $0 }
        )
        let coordinator = field.makeCoordinator()
        let control = PaletteNSTextField()
        control.stringValue = "검색"

        coordinator.controlTextDidChange(Notification(
            name: NSControl.textDidChangeNotification,
            object: control
        ))

        #expect(text.value == "검색")
        #expect(query.value == "검색")
    }

    @Test("does not submit while IME marked text is active")
    func doesNotSubmitWhileIMEMarkedTextIsActive() {
        let text = PaletteSearchFieldTextBox()
        let submitted = PaletteSearchFieldTextBox()
        let field = PaletteSearchField(
            text: Binding(
                get: { text.value },
                set: { text.value = $0 }
            ),
            placeholder: "Search",
            onSubmit: {},
            onSubmitText: { submitted.value = $0 },
            onEscape: {},
            onArrowUp: {},
            onArrowDown: {}
        )
        let coordinator = field.makeCoordinator()
        let control = PaletteNSTextField()
        control.stringValue = "rja"

        let handled = coordinator.control(
            control,
            textView: MarkedTextView(),
            doCommandBy: #selector(NSResponder.insertNewline(_:))
        )

        #expect(!handled)
        #expect(text.value.isEmpty)
        #expect(submitted.value.isEmpty)
        #expect(control.consumeSubmitAfterMarkedTextCommit())
    }

    @Test("invokes page navigation commands")
    func invokesPageNavigationCommands() {
        let pageUp = PaletteSearchFieldFlag()
        let pageDown = PaletteSearchFieldFlag()
        let text = PaletteSearchFieldTextBox()
        let field = PaletteSearchField(
            text: Binding(
                get: { text.value },
                set: { text.value = $0 }
            ),
            placeholder: "Search",
            onSubmit: {},
            onEscape: {},
            onArrowUp: {},
            onArrowDown: {},
            onPageUp: { pageUp.value = true },
            onPageDown: { pageDown.value = true }
        )
        let coordinator = field.makeCoordinator()
        let control = NSTextField()

        let handledPageUp = coordinator.control(
            control,
            textView: NSTextView(),
            doCommandBy: #selector(NSResponder.pageUp(_:))
        )
        let handledPageDown = coordinator.control(
            control,
            textView: NSTextView(),
            doCommandBy: #selector(NSResponder.pageDown(_:))
        )

        #expect(handledPageUp)
        #expect(handledPageDown)
        #expect(pageUp.value)
        #expect(pageDown.value)
    }

    @Test("reports focus changes")
    func reportsFocusChanges() {
        let focused = PaletteSearchFieldFlag()
        let text = PaletteSearchFieldTextBox()
        let field = PaletteSearchField(
            text: Binding(
                get: { text.value },
                set: { text.value = $0 }
            ),
            placeholder: "Search",
            onSubmit: {},
            onEscape: {},
            onArrowUp: {},
            onArrowDown: {},
            onFocusChange: { focused.value = $0 }
        )
        let coordinator = field.makeCoordinator()

        coordinator.controlTextDidBeginEditing(Notification(name: NSControl.textDidBeginEditingNotification))
        #expect(focused.value)

        coordinator.controlTextDidEndEditing(Notification(name: NSControl.textDidEndEditingNotification))
        #expect(!focused.value)
    }

    @Test("overlay key monitor does not consume return while field editor is first responder")
    func overlayKeyMonitorDoesNotConsumeReturnWhileFieldEditorIsFirstResponder() {
        let fieldEditor = NSTextView()

        let action = PaletteOverlayKeyboard.action(
            forKeyCode: 36,
            firstResponder: fieldEditor,
            isSearchFieldFocused: false
        )

        #expect(action == nil)
    }

    @Test("overlay key monitor confirms selection on return outside field editor")
    func overlayKeyMonitorConfirmsSelectionOnReturnOutsideFieldEditor() {
        let action = PaletteOverlayKeyboard.action(
            forKeyCode: 36,
            firstResponder: nil,
            isSearchFieldFocused: false
        )

        #expect(action == .confirmSelection)
    }

    @Test("overlay key monitor does not confirm selection on return while search field is focused")
    func overlayKeyMonitorDoesNotConfirmReturnWhileSearchFieldFocused() {
        let action = PaletteOverlayKeyboard.action(
            forKeyCode: 36,
            firstResponder: nil,
            isSearchFieldFocused: true
        )

        #expect(action == nil)
    }

    @Test("overlay key monitor maps page up and page down key codes")
    func overlayKeyMonitorMapsPageNavigationKeyCodes() {
        let pageUp = PaletteOverlayKeyboard.action(
            forKeyCode: 116,
            firstResponder: nil,
            isSearchFieldFocused: true
        )
        let pageDown = PaletteOverlayKeyboard.action(
            forKeyCode: 121,
            firstResponder: nil,
            isSearchFieldFocused: true
        )

        #expect(pageUp == .pageUp)
        #expect(pageDown == .pageDown)
    }

    private func waitForFocus(_ field: NSTextField) async throws {
        for _ in 0..<40 {
            if field.currentEditor() != nil {
                return
            }
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(field.currentEditor() != nil)
    }

    private func textField(in view: NSView) -> NSTextField? {
        if let field = view as? NSTextField {
            return field
        }
        for subview in view.subviews {
            if let field = textField(in: subview) {
                return field
            }
        }
        return nil
    }
}

@MainActor
private final class PaletteSearchFieldTextBox {
    var value = ""
}

private final class MarkedTextView: NSTextView {
    override func hasMarkedText() -> Bool {
        true
    }
}

@MainActor
private final class PaletteSearchFieldFlag {
    var value = false
}
