import AppKit
import SwiftUI
import Testing

@testable import Muxy

@MainActor
@Suite("BrowserAddressField")
struct BrowserAddressFieldTests {
    @Test("does not submit when editing ends")
    func doesNotSubmitWhenEditingEnds() throws {
        let text = BrowserAddressFieldTextBox()
        let focus = BrowserAddressFieldFocusBox(value: false)
        let view = BrowserAddressField(
            text: Binding(
                get: { text.value },
                set: { text.value = $0 }
            ),
            isFocused: Binding(
                get: { focus.value },
                set: { focus.value = $0 }
            ),
            model: BrowserSuggestionModel(),
            suggestionsProvider: { _ in [] },
            onFocusClaimed: {},
            onSubmit: { _, _ in }
        )
        let hostingView = NSHostingView(rootView: view)
        hostingView.layoutSubtreeIfNeeded()

        let field = try #require(textField(in: hostingView))
        #expect(field.cell?.sendsActionOnEndEditing == false)
    }

    @Test("claims focus after late window attachment")
    func claimsFocusAfterLateWindowAttachment() async throws {
        let text = BrowserAddressFieldTextBox()
        let focus = BrowserAddressFieldFocusBox(value: true)
        let view = BrowserAddressField(
            text: Binding(
                get: { text.value },
                set: { text.value = $0 }
            ),
            isFocused: Binding(
                get: { focus.value },
                set: { focus.value = $0 }
            ),
            model: BrowserSuggestionModel(),
            suggestionsProvider: { _ in [] },
            onFocusClaimed: {},
            onSubmit: { _, _ in }
        )
        .frame(width: 320, height: 28)
        let hostingView = NSHostingView(rootView: view)
        hostingView.frame = NSRect(x: 0, y: 0, width: 320, height: 28)
        hostingView.layoutSubtreeIfNeeded()

        let field = try #require(textField(in: hostingView))
        #expect(field.window == nil)

        try await Task.sleep(for: .milliseconds(50))

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 360, height: 120),
            styleMask: [.titled],
            backing: .buffered,
            defer: false
        )
        defer { window.orderOut(nil) }
        window.contentView = hostingView
        window.makeKeyAndOrderFront(nil)

        try await waitForFocus(field)
    }

    @Test("canceled focus request does not claim focus after late window attachment")
    func canceledFocusRequestDoesNotClaimFocusAfterLateWindowAttachment() async throws {
        let text = BrowserAddressFieldTextBox()
        let focus = BrowserAddressFieldFocusBox(value: true)
        let coordinator = BrowserAddressField.Coordinator(
            text: Binding(
                get: { text.value },
                set: { text.value = $0 }
            ),
            isFocused: Binding(
                get: { focus.value },
                set: { focus.value = $0 }
            ),
            model: BrowserSuggestionModel(),
            suggestionsProvider: { _ in [] },
            onFocusClaimed: {},
            onSubmit: { _, _ in }
        )
        let field = NSTextField()
        coordinator.field = field
        coordinator.applyFocus(true)
        focus.value = false
        coordinator.applyFocus(false)

        let focusSink = BrowserAddressFieldFocusSink()
        let container = NSView(frame: NSRect(x: 0, y: 0, width: 360, height: 120))
        focusSink.frame = NSRect(x: 0, y: 0, width: 1, height: 1)
        field.frame = NSRect(x: 10, y: 50, width: 320, height: 28)
        container.addSubview(focusSink)
        container.addSubview(field)

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 360, height: 120),
            styleMask: [.titled],
            backing: .buffered,
            defer: false
        )
        defer { window.orderOut(nil) }
        window.contentView = container
        window.initialFirstResponder = focusSink
        window.makeKeyAndOrderFront(nil)
        window.makeFirstResponder(focusSink)

        try await Task.sleep(for: .milliseconds(250))
        #expect(field.currentEditor() == nil)
    }

    @Test("return submits latest field text")
    func returnSubmitsLatestFieldText() throws {
        let text = BrowserAddressFieldTextBox()
        text.value = "https://old.example"
        let submitted = BrowserAddressFieldSubmissionBox()
        let coordinator = BrowserAddressField.Coordinator(
            text: Binding(
                get: { text.value },
                set: { text.value = $0 }
            ),
            isFocused: Binding.constant(false),
            model: BrowserSuggestionModel(),
            suggestionsProvider: { _ in [] },
            onFocusClaimed: {},
            onSubmit: { entry, value in submitted.value = (entry, value) }
        )
        let field = NSTextField()
        field.stringValue = "https://typed.example"
        coordinator.field = field

        coordinator.handleSubmit()

        let value = try #require(submitted.value)
        #expect(value.entry == nil)
        #expect(value.text == "https://typed.example")
        #expect(text.value == "https://typed.example")
    }

    @Test("return submits keyboard selected history entry")
    func returnSubmitsKeyboardSelectedHistoryEntry() throws {
        let profileID = UUID()
        let entries = [
            BrowserHistoryEntry(
                profileID: profileID,
                url: "https://first.example",
                lastVisited: Date()
            ),
            BrowserHistoryEntry(
                profileID: profileID,
                url: "https://second.example",
                lastVisited: Date()
            ),
        ]
        let model = BrowserSuggestionModel()
        model.update(entries)
        model.moveSelection(1)
        model.hover(entries[1])
        let submitted = BrowserAddressFieldSubmissionBox()
        let coordinator = BrowserAddressField.Coordinator(
            text: Binding.constant("typed"),
            isFocused: Binding.constant(false),
            model: model,
            suggestionsProvider: { _ in [] },
            onFocusClaimed: {},
            onSubmit: { entry, value in submitted.value = (entry, value) }
        )
        let field = NSTextField()
        field.stringValue = "typed"
        coordinator.field = field

        coordinator.handleSubmit()

        let value = try #require(submitted.value)
        #expect(value.entry?.url == "https://first.example")
        #expect(value.text == "typed")
    }

    @Test("return command submits active history entry")
    func returnCommandSubmitsActiveHistoryEntry() throws {
        let profileID = UUID()
        let entries = [
            BrowserHistoryEntry(
                profileID: profileID,
                url: "https://first.example",
                lastVisited: Date()
            ),
            BrowserHistoryEntry(
                profileID: profileID,
                url: "https://second.example",
                lastVisited: Date()
            ),
        ]
        let model = BrowserSuggestionModel()
        model.update(entries)
        model.moveSelection(1)
        let submitted = BrowserAddressFieldSubmissionBox()
        let coordinator = BrowserAddressField.Coordinator(
            text: Binding.constant("typed"),
            isFocused: Binding.constant(false),
            model: model,
            suggestionsProvider: { _ in [] },
            onFocusClaimed: {},
            onSubmit: { entry, value in submitted.value = (entry, value) }
        )
        let field = NSTextField()
        field.stringValue = "typed"
        coordinator.field = field

        let handled = coordinator.control(
            field,
            textView: NSTextView(),
            doCommandBy: #selector(NSResponder.insertNewline(_:))
        )

        let value = try #require(submitted.value)
        #expect(handled)
        #expect(value.entry?.url == "https://first.example")
        #expect(value.text == "typed")
    }

    @Test("return submits hovered history entry when keyboard selection is empty")
    func returnSubmitsHoveredHistoryEntryWhenKeyboardSelectionIsEmpty() throws {
        let profileID = UUID()
        let entries = [
            BrowserHistoryEntry(
                profileID: profileID,
                url: "https://first.example",
                lastVisited: Date()
            ),
            BrowserHistoryEntry(
                profileID: profileID,
                url: "https://second.example",
                lastVisited: Date()
            ),
        ]
        let model = BrowserSuggestionModel()
        model.update(entries)
        model.hover(entries[1])
        let submitted = BrowserAddressFieldSubmissionBox()
        let coordinator = BrowserAddressField.Coordinator(
            text: Binding.constant("typed"),
            isFocused: Binding.constant(false),
            model: model,
            suggestionsProvider: { _ in [] },
            onFocusClaimed: {},
            onSubmit: { entry, value in submitted.value = (entry, value) }
        )
        let field = NSTextField()
        field.stringValue = "typed"
        coordinator.field = field

        coordinator.handleSubmit()

        let value = try #require(submitted.value)
        #expect(value.entry?.url == "https://second.example")
        #expect(value.text == "typed")
    }

    @Test("pending address focus keeps web view unfocused")
    func pendingAddressFocusKeepsWebViewUnfocused() {
        #expect(!BrowserPane.shouldFocusWebView(
            paneFocused: true,
            addressFieldFocused: false,
            findFieldFocused: false,
            addressFocusPending: true
        ))
    }

    @Test("web view focuses only when pane focus is clear")
    func webViewFocusRequiresClearPaneFocus() {
        #expect(BrowserPane.shouldFocusWebView(
            paneFocused: true,
            addressFieldFocused: false,
            findFieldFocused: false,
            addressFocusPending: false
        ))
        #expect(!BrowserPane.shouldFocusWebView(
            paneFocused: false,
            addressFieldFocused: false,
            findFieldFocused: false,
            addressFocusPending: false
        ))
        #expect(!BrowserPane.shouldFocusWebView(
            paneFocused: true,
            addressFieldFocused: true,
            findFieldFocused: false,
            addressFocusPending: false
        ))
        #expect(!BrowserPane.shouldFocusWebView(
            paneFocused: true,
            addressFieldFocused: false,
            findFieldFocused: true,
            addressFocusPending: false
        ))
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
private final class BrowserAddressFieldTextBox {
    var value = ""
}

@MainActor
private final class BrowserAddressFieldFocusBox {
    var value: Bool

    init(value: Bool) {
        self.value = value
    }
}

private final class BrowserAddressFieldFocusSink: NSView {
    override var acceptsFirstResponder: Bool { true }
}

@MainActor
private final class BrowserAddressFieldSubmissionBox {
    var value: (entry: BrowserHistoryEntry?, text: String)?
}
