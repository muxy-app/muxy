import AppKit
import SwiftUI

@MainActor
@Observable
final class ModifierKeyMonitor {
    static let shared = ModifierKeyMonitor()

    private(set) var commandHeld = false
    private(set) var controlHeld = false
    private var monitor: Any?

    private init() {}

    func start() {
        guard monitor == nil else { return }
        monitor = NSEvent.addLocalMonitorForEvents(matching: .flagsChanged) { [weak self] event in
            guard let self else { return event }
            let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
            MainActor.assumeIsolated {
                self.commandHeld = flags.contains(.command)
                self.controlHeld = flags.contains(.control)
            }
            return event
        }
    }

    func stop() {
        guard let monitor else { return }
        NSEvent.removeMonitor(monitor)
        self.monitor = nil
        commandHeld = false
        controlHeld = false
    }
}
