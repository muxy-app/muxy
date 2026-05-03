import AppKit
import SwiftUI

struct DoubleClickView: NSViewRepresentable {
    let action: () -> Void

    func makeNSView(context: Context) -> DoubleClickNSView {
        let view = DoubleClickNSView()
        view.action = action
        return view
    }

    func updateNSView(_ nsView: DoubleClickNSView, context: Context) {
        nsView.action = action
    }
}

final class DoubleClickNSView: NSView {
    var action: (() -> Void)?

    override func isAccessibilityElement() -> Bool {
        false
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        guard let currentEvent = NSApp.currentEvent,
              currentEvent.type == .leftMouseDown,
              currentEvent.clickCount == 2
        else { return nil }
        return super.hitTest(point)
    }

    override func mouseDown(with event: NSEvent) {
        guard event.clickCount == 2 else {
            super.mouseDown(with: event)
            return
        }
        action?()
    }
}
