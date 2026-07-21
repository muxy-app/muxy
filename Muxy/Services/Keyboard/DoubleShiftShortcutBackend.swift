import AppKit
import CoreGraphics

private let quickTerminalDoubleShiftEventTapCallback: CGEventTapCallBack = { _, type, event, userData in
    guard let userData else { return Unmanaged.passUnretained(event) }
    let flags = event.flags
    let timestamp = TimeInterval(event.timestamp) / 1_000_000_000
    let backend = Unmanaged<DoubleShiftShortcutBackend>.fromOpaque(userData).takeUnretainedValue()
    MainActor.assumeIsolated {
        backend.receiveGlobalEvent(type: type, flags: flags, timestamp: timestamp)
    }
    return Unmanaged.passUnretained(event)
}

@MainActor
final class DoubleShiftShortcutBackend: QuickTerminalShortcutBackend {
    private static let otherAppKitModifiers: NSEvent.ModifierFlags = [
        .control,
        .option,
        .command,
        .function,
    ]
    private static let otherCGEventModifiers: CGEventFlags = [
        .maskControl,
        .maskAlternate,
        .maskCommand,
        .maskSecondaryFn,
    ]

    private var detector: DoubleShiftDetector
    private var modifierState = DoubleShiftModifierState()
    private var localMonitor: Any?
    private var eventTap: CFMachPort?
    private var eventTapSource: CFRunLoopSource?
    private var trigger: (@MainActor () -> Void)?

    init(detector: DoubleShiftDetector = DoubleShiftDetector()) {
        self.detector = detector
    }

    deinit {
        MainActor.assumeIsolated {
            stop()
        }
    }

    var monitoringState: QuickTerminalShortcutMonitoringState {
        if eventTap != nil {
            return .systemWide
        }
        if localMonitor != nil {
            return .localOnly
        }
        return .stopped
    }

    static var hasInputMonitoringAccess: Bool {
        CGPreflightListenEventAccess()
    }

    static func requestInputMonitoringAccess() -> Bool {
        CGRequestListenEventAccess()
    }

    func start(trigger: @escaping @MainActor () -> Void) throws {
        guard localMonitor == nil else { return }
        self.trigger = trigger
        localMonitor = NSEvent.addLocalMonitorForEvents(
            matching: [.flagsChanged, .keyDown, .leftMouseDown, .rightMouseDown, .otherMouseDown]
        ) { [weak self] event in
            MainActor.assumeIsolated {
                self?.receiveLocalEvent(event)
            }
            return event
        }
        _ = enableSystemWideMonitoringIfAuthorized()
    }

    func stop() {
        if let localMonitor {
            NSEvent.removeMonitor(localMonitor)
        }
        removeEventTap()
        localMonitor = nil
        trigger = nil
        detector.reset()
        modifierState.reset()
    }

    @discardableResult
    func enableSystemWideMonitoringIfAuthorized() -> Bool {
        guard eventTap == nil else { return true }
        guard Self.hasInputMonitoringAccess else { return false }

        let mask = CGEventMask(1 << CGEventType.flagsChanged.rawValue)
            | CGEventMask(1 << CGEventType.keyDown.rawValue)
            | CGEventMask(1 << CGEventType.leftMouseDown.rawValue)
            | CGEventMask(1 << CGEventType.rightMouseDown.rawValue)
            | CGEventMask(1 << CGEventType.otherMouseDown.rawValue)
        guard let eventTap = CGEvent.tapCreate(
            tap: .cgSessionEventTap,
            place: .headInsertEventTap,
            options: .listenOnly,
            eventsOfInterest: mask,
            callback: quickTerminalDoubleShiftEventTapCallback,
            userInfo: Unmanaged.passUnretained(self).toOpaque()
        ), let eventTapSource = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, eventTap, 0)
        else { return false }

        self.eventTap = eventTap
        self.eventTapSource = eventTapSource
        CFRunLoopAddSource(CFRunLoopGetMain(), eventTapSource, .commonModes)
        CGEvent.tapEnable(tap: eventTap, enable: true)
        return true
    }

    func receiveGlobalEvent(type: CGEventType, flags: CGEventFlags, timestamp: TimeInterval) {
        if type == .tapDisabledByTimeout || type == .tapDisabledByUserInput {
            if let eventTap {
                CGEvent.tapEnable(tap: eventTap, enable: true)
            }
            return
        }
        guard NSApp?.isActive != true else { return }

        switch type {
        case .flagsChanged:
            let conventionalModifierPressed = !flags.isDisjoint(with: Self.otherCGEventModifiers)
            process(.modifierChange(
                shiftPressed: flags.contains(.maskShift),
                otherModifierPressed: modifierState.otherModifierPressed(
                    conventionalModifierPressed: conventionalModifierPressed,
                    capsLockEnabled: flags.contains(.maskAlphaShift)
                ),
                timestamp: timestamp
            ))
        case .keyDown:
            process(.keyDown(shiftPressed: flags.contains(.maskShift), timestamp: timestamp))
        case .leftMouseDown,
             .rightMouseDown,
             .otherMouseDown:
            process(.pointerDown(shiftPressed: flags.contains(.maskShift), timestamp: timestamp))
        default:
            break
        }
    }

    private func receiveLocalEvent(_ event: NSEvent) {
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        switch event.type {
        case .flagsChanged:
            let conventionalModifierPressed = !flags.isDisjoint(with: Self.otherAppKitModifiers)
            process(.modifierChange(
                shiftPressed: flags.contains(.shift),
                otherModifierPressed: modifierState.otherModifierPressed(
                    conventionalModifierPressed: conventionalModifierPressed,
                    capsLockEnabled: flags.contains(.capsLock)
                ),
                timestamp: event.timestamp
            ))
        case .keyDown:
            process(.keyDown(shiftPressed: flags.contains(.shift), timestamp: event.timestamp))
        case .leftMouseDown,
             .rightMouseDown,
             .otherMouseDown:
            process(.pointerDown(shiftPressed: flags.contains(.shift), timestamp: event.timestamp))
        default:
            break
        }
    }

    private func process(_ input: DoubleShiftDetector.Input) {
        guard detector.process(input) else { return }
        trigger?()
    }

    private func removeEventTap() {
        if let eventTapSource {
            CFRunLoopRemoveSource(CFRunLoopGetMain(), eventTapSource, .commonModes)
        }
        if let eventTap {
            CFMachPortInvalidate(eventTap)
        }
        eventTapSource = nil
        eventTap = nil
    }
}
