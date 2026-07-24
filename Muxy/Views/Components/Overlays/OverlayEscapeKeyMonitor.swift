import AppKit
import Carbon.HIToolbox
import SwiftUI

struct OverlayEscapeKeyMonitor: ViewModifier {
    let action: () -> Void
    @State private var monitor: Any?

    func body(content: Content) -> some View {
        content
            .onAppear(perform: install)
            .onDisappear(perform: remove)
    }

    private func install() {
        guard monitor == nil else { return }
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [action] event in
            OverlayEscapeKeyHandler.handle(event, action: action)
        }
    }

    private func remove() {
        guard let monitor else { return }
        NSEvent.removeMonitor(monitor)
        self.monitor = nil
    }
}

enum OverlayEscapeKeyHandler {
    static func handle(_ event: NSEvent, action: () -> Void) -> NSEvent? {
        guard event.keyCode == UInt16(kVK_Escape) else { return event }
        action()
        return nil
    }
}

extension View {
    func overlayEscapeKeyMonitor(action: @escaping () -> Void) -> some View {
        modifier(OverlayEscapeKeyMonitor(action: action))
    }
}
