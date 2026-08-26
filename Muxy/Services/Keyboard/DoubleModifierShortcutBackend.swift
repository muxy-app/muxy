import AppKit
import CoreGraphics

private let doubleModifierShortcutEventTapCallback: CGEventTapCallBack = { _, type, event, userData in
    guard let userData else { return Unmanaged.passUnretained(event) }
    let flags = event.flags
    let timestamp = TimeInterval(event.timestamp) / 1_000_000_000
    let backend = Unmanaged<DoubleModifierShortcutBackend>.fromOpaque(userData).takeUnretainedValue()
    MainActor.assumeIsolated {
        backend.receiveGlobalEvent(type: type, flags: flags, timestamp: timestamp)
    }
    return Unmanaged.passUnretained(event)
}

struct DoubleModifierTapDetector {
    struct Configuration: Equatable {
        var maximumTapDuration: TimeInterval
        var maximumInterval: TimeInterval

        init(maximumTapDuration: TimeInterval = 0.3, maximumInterval: TimeInterval = 0.3) {
            self.maximumTapDuration = maximumTapDuration
            self.maximumInterval = maximumInterval
        }
    }

    enum Input: Equatable {
        case modifierChange(modifierPressed: Bool, otherModifierPressed: Bool, timestamp: TimeInterval)
        case keyDown(modifierPressed: Bool, timestamp: TimeInterval)
        case pointerDown(modifierPressed: Bool, timestamp: TimeInterval)

        var timestamp: TimeInterval {
            switch self {
            case let .modifierChange(_, _, timestamp),
                 let .keyDown(_, timestamp),
                 let .pointerDown(_, timestamp): timestamp
            }
        }

        var modifierPressed: Bool {
            switch self {
            case let .modifierChange(modifierPressed, _, _),
                 let .keyDown(modifierPressed, _),
                 let .pointerDown(modifierPressed, _): modifierPressed
            }
        }
    }

    private enum State: Equatable {
        case idle
        case firstPress(TimeInterval)
        case awaitingSecondPress(TimeInterval)
        case secondPress(TimeInterval)
        case blockedUntilRelease
    }

    private let configuration: Configuration
    private var state = State.idle
    private var lastTimestamp: TimeInterval?

    init(configuration: Configuration = Configuration()) {
        self.configuration = configuration
    }

    mutating func process(_ input: Input) -> Bool {
        if let lastTimestamp, input.timestamp < lastTimestamp {
            state = .idle
        }
        lastTimestamp = input.timestamp

        switch input {
        case let .keyDown(modifierPressed, _),
             let .pointerDown(modifierPressed, _):
            state = modifierPressed ? .blockedUntilRelease : .idle
            return false
        case let .modifierChange(modifierPressed, otherModifierPressed, timestamp):
            return processModifierChange(
                modifierPressed: modifierPressed,
                otherModifierPressed: otherModifierPressed,
                timestamp: timestamp
            )
        }
    }

    mutating func reset() {
        state = .idle
        lastTimestamp = nil
    }

    private mutating func processModifierChange(
        modifierPressed: Bool,
        otherModifierPressed: Bool,
        timestamp: TimeInterval
    ) -> Bool {
        if otherModifierPressed {
            state = modifierPressed ? .blockedUntilRelease : .idle
            return false
        }

        if case .blockedUntilRelease = state {
            if !modifierPressed {
                state = .idle
            }
            return false
        }

        expireState(at: timestamp, modifierPressed: modifierPressed)

        if modifierPressed {
            switch state {
            case .idle:
                state = .firstPress(timestamp)
            case .awaitingSecondPress:
                state = .secondPress(timestamp)
            case .firstPress,
                 .secondPress,
                 .blockedUntilRelease:
                break
            }
            return false
        }

        switch state {
        case let .firstPress(pressedAt):
            guard timestamp - pressedAt <= configuration.maximumTapDuration else {
                state = .idle
                return false
            }
            state = .awaitingSecondPress(timestamp)
            return false
        case let .secondPress(pressedAt):
            state = .idle
            return timestamp - pressedAt <= configuration.maximumTapDuration
        case .idle,
             .awaitingSecondPress,
             .blockedUntilRelease:
            return false
        }
    }

    private mutating func expireState(at timestamp: TimeInterval, modifierPressed: Bool) {
        switch state {
        case let .firstPress(pressedAt),
             let .secondPress(pressedAt):
            guard timestamp - pressedAt > configuration.maximumTapDuration else { return }
            state = modifierPressed ? .blockedUntilRelease : .idle
        case let .awaitingSecondPress(releasedAt):
            guard timestamp - releasedAt > configuration.maximumInterval else { return }
            state = .idle
        case .idle,
             .blockedUntilRelease:
            break
        }
    }
}

@MainActor
final class DoubleModifierShortcutBackend: QuickTerminalShortcutBackend {
    enum Modifier: Equatable {
        case shift
        case command
        case control
        case option

        var appKitFlag: NSEvent.ModifierFlags {
            switch self {
            case .shift: .shift
            case .command: .command
            case .control: .control
            case .option: .option
            }
        }

        var cgEventFlag: CGEventFlags {
            switch self {
            case .shift: .maskShift
            case .command: .maskCommand
            case .control: .maskControl
            case .option: .maskAlternate
            }
        }
    }

    private let modifier: Modifier
    private var detector: DoubleModifierTapDetector
    private var capsLockEnabled: Bool?
    private var localMonitor: Any?
    private var eventTap: CFMachPort?
    private var eventTapSource: CFRunLoopSource?
    private var trigger: (@MainActor () -> Void)?

    init(
        modifier: Modifier,
        maximumTapDuration: TimeInterval = 0.3,
        maximumInterval: TimeInterval = 0.3
    ) {
        self.modifier = modifier
        detector = DoubleModifierTapDetector(configuration: .init(
            maximumTapDuration: maximumTapDuration,
            maximumInterval: maximumInterval
        ))
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
        capsLockEnabled = nil
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
            callback: doubleModifierShortcutEventTapCallback,
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
            process(.modifierChange(
                modifierPressed: flags.contains(modifier.cgEventFlag),
                otherModifierPressed: otherModifierPressed(flags),
                timestamp: timestamp
            ))
        case .keyDown:
            process(.keyDown(modifierPressed: flags.contains(modifier.cgEventFlag), timestamp: timestamp))
        case .leftMouseDown,
             .rightMouseDown,
             .otherMouseDown:
            process(.pointerDown(modifierPressed: flags.contains(modifier.cgEventFlag), timestamp: timestamp))
        default:
            break
        }
    }

    private func receiveLocalEvent(_ event: NSEvent) {
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        switch event.type {
        case .flagsChanged:
            process(.modifierChange(
                modifierPressed: flags.contains(modifier.appKitFlag),
                otherModifierPressed: otherModifierPressed(flags),
                timestamp: event.timestamp
            ))
        case .keyDown:
            process(.keyDown(modifierPressed: flags.contains(modifier.appKitFlag), timestamp: event.timestamp))
        case .leftMouseDown,
             .rightMouseDown,
             .otherMouseDown:
            process(.pointerDown(modifierPressed: flags.contains(modifier.appKitFlag), timestamp: event.timestamp))
        default:
            break
        }
    }

    private func process(_ input: DoubleModifierTapDetector.Input) {
        guard detector.process(input) else { return }
        trigger?()
    }

    private func otherModifierPressed(_ flags: NSEvent.ModifierFlags) -> Bool {
        let conventional: NSEvent.ModifierFlags = [.shift, .command, .control, .option, .function]
        let otherFlags = conventional.subtracting(modifier.appKitFlag)
        let capsLockChanged = capsLockEnabled.map { $0 != flags.contains(.capsLock) } ?? false
        capsLockEnabled = flags.contains(.capsLock)
        return !flags.isDisjoint(with: otherFlags) || capsLockChanged
    }

    private func otherModifierPressed(_ flags: CGEventFlags) -> Bool {
        let conventional: CGEventFlags = [.maskShift, .maskCommand, .maskControl, .maskAlternate, .maskSecondaryFn]
        var otherFlags = conventional
        otherFlags.remove(modifier.cgEventFlag)
        let capsLockChanged = capsLockEnabled.map { $0 != flags.contains(.maskAlphaShift) } ?? false
        capsLockEnabled = flags.contains(.maskAlphaShift)
        return !flags.isDisjoint(with: otherFlags) || capsLockChanged
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
