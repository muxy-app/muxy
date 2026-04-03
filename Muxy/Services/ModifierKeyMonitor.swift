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
    private var activationObserver: NSObjectProtocol?
    private var hintTimer: Timer?

    private static let hintDelay: TimeInterval = 0.5

    private init() {}

    func start() {
        guard monitor == nil else { return }
        monitor = NSEvent.addLocalMonitorForEvents(matching: .flagsChanged) { [weak self] event in
            guard let self else { return event }
            let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
            MainActor.assumeIsolated {
                self.updateFlags(flags)
            }
            return event
        }
        activationObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.didBecomeActiveNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self else { return }
                let flags = NSEvent.modifierFlags.intersection(.deviceIndependentFlagsMask)
                self.updateFlags(flags)
            }
        }
    }

    func stop() {
        if let monitor {
            NSEvent.removeMonitor(monitor)
        }
        if let activationObserver {
            NotificationCenter.default.removeObserver(activationObserver)
        }
        monitor = nil
        activationObserver = nil
        cancelHint()
        commandHeld = false
        controlHeld = false
        shiftHeld = false
        optionHeld = false
    }

    func isHolding(modifiers: UInt) -> Bool {
        guard showHints else { return false }
        let flags = NSEvent.ModifierFlags(rawValue: modifiers).intersection(.deviceIndependentFlagsMask)
        if flags.contains(.command), !commandHeld { return false }
        if flags.contains(.control), !controlHeld { return false }
        if flags.contains(.shift), !shiftHeld { return false }
        if flags.contains(.option), !optionHeld { return false }
        guard !flags.isEmpty else { return false }
        return true
    }

    private func updateFlags(_ flags: NSEvent.ModifierFlags) {
        let wasHoldingModifier = commandHeld || controlHeld
        commandHeld = flags.contains(.command)
        controlHeld = flags.contains(.control)
        shiftHeld = flags.contains(.shift)
        optionHeld = flags.contains(.option)
        let isHoldingModifier = commandHeld || controlHeld
        if isHoldingModifier, !wasHoldingModifier {
            scheduleHint()
        } else if !isHoldingModifier {
            cancelHint()
        }
    }

    private func scheduleHint() {
        hintTimer?.invalidate()
        hintTimer = Timer.scheduledTimer(withTimeInterval: Self.hintDelay, repeats: false) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self, self.commandHeld || self.controlHeld else { return }
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
