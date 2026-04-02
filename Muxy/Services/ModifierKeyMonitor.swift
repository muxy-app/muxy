import AppKit
import SwiftUI

@MainActor
@Observable
final class ModifierKeyMonitor {
    static let shared = ModifierKeyMonitor()

    private(set) var commandHeld = false
    private(set) var controlHeld = false
    private(set) var shiftHeld = false
    private(set) var optionHeld = false
    private(set) var showHints = false
    private var monitor: Any?
    private var hintTimer: Timer?

    private static let hintDelay: TimeInterval = 0.5

    private init() {}

    func start() {
        guard monitor == nil else { return }
        monitor = NSEvent.addLocalMonitorForEvents(matching: .flagsChanged) { [weak self] event in
            guard let self else { return event }
            let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
            MainActor.assumeIsolated {
                let wasHoldingModifier = self.commandHeld || self.controlHeld
                self.commandHeld = flags.contains(.command)
                self.controlHeld = flags.contains(.control)
                self.shiftHeld = flags.contains(.shift)
                self.optionHeld = flags.contains(.option)
                let isHoldingModifier = self.commandHeld || self.controlHeld
                if isHoldingModifier && !wasHoldingModifier {
                    self.scheduleHint()
                } else if !isHoldingModifier {
                    self.cancelHint()
                }
            }
            return event
        }
    }

    func stop() {
        guard let monitor else { return }
        NSEvent.removeMonitor(monitor)
        self.monitor = nil
        cancelHint()
        commandHeld = false
        controlHeld = false
        shiftHeld = false
        optionHeld = false
    }

    func isHolding(modifiers: UInt) -> Bool {
        guard showHints else { return false }
        let flags = NSEvent.ModifierFlags(rawValue: modifiers).intersection(.deviceIndependentFlagsMask)
        if flags.contains(.command) && !commandHeld { return false }
        if flags.contains(.control) && !controlHeld { return false }
        if flags.contains(.shift) && !shiftHeld { return false }
        if flags.contains(.option) && !optionHeld { return false }
        guard !flags.isEmpty else { return false }
        return true
    }

    private func scheduleHint() {
        hintTimer?.invalidate()
        hintTimer = Timer.scheduledTimer(withTimeInterval: Self.hintDelay, repeats: false) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self, (self.commandHeld || self.controlHeld) else { return }
                self.showHints = true
            }
        }
    }

    private func cancelHint() {
        hintTimer?.invalidate()
        hintTimer = nil
        showHints = false
    }
}
