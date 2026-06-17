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

@MainActor
private final class PaletteSearchFieldFlag {
    var value = false
}
