import AppKit
import SwiftUI

struct LeftClickView: NSViewRepresentable {
    let action: () -> Void

    func makeNSView(context: Context) -> LeftClickNSView {
        let view = LeftClickNSView()
        view.action = action
        return view
    }

    func updateNSView(_ nsView: LeftClickNSView, context: Context) {
        nsView.action = action
    }
}

final class LeftClickNSView: NSView {
    var action: (() -> Void)?

    override func isAccessibilityElement() -> Bool {
        false
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        guard let currentEvent = NSApp.currentEvent,
              currentEvent.type == .leftMouseDown
        else { return nil }
        return super.hitTest(point)
    }

    override func mouseDown(with event: NSEvent) {}

    override func mouseUp(with event: NSEvent) {
        guard bounds.contains(convert(event.locationInWindow, from: nil)) else { return }
        action?()
    }
}
