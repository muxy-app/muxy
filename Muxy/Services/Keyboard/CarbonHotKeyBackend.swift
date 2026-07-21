import AppKit
import Carbon

private let quickTerminalCarbonEventHandler: EventHandlerUPP = { _, event, userData in
    guard let event, let userData else { return OSStatus(eventNotHandledErr) }
    var hotKeyID = EventHotKeyID()
    let status = GetEventParameter(
        event,
        EventParamName(kEventParamDirectObject),
        EventParamType(typeEventHotKeyID),
        nil,
        MemoryLayout<EventHotKeyID>.size,
        nil,
        &hotKeyID
    )
    guard status == noErr else { return OSStatus(eventNotHandledErr) }
    let signature = hotKeyID.signature
    let identifier = hotKeyID.id
    let backend = Unmanaged<CarbonHotKeyBackend>.fromOpaque(userData).takeUnretainedValue()
    return MainActor.assumeIsolated {
        backend.handle(signature: signature, identifier: identifier)
    }
}

@MainActor
final class CarbonHotKeyBackend: QuickTerminalShortcutBackend {
    private static let signature: OSType = 0x4D58_4E54
    private static var nextIdentifier: UInt32 = 1

    let combo: KeyCombo
    let virtualKeyCode: UInt16
    private let identifier: UInt32
    private var eventHandler: EventHandlerRef?
    private var hotKey: EventHotKeyRef?
    private var trigger: (@MainActor () -> Void)?

    init(combo: KeyCombo, virtualKeyCode: UInt16) {
        self.combo = combo
        self.virtualKeyCode = virtualKeyCode
        identifier = Self.nextIdentifier
        Self.nextIdentifier &+= 1
    }

    deinit {
        MainActor.assumeIsolated {
            stop()
        }
    }

    var monitoringState: QuickTerminalShortcutMonitoringState {
        hotKey == nil ? .stopped : .carbonHotKey
    }

    func start(trigger: @escaping @MainActor () -> Void) throws {
        guard hotKey == nil else { return }
        guard QuickTerminalShortcut.keyCombo(combo, virtualKeyCode: virtualKeyCode).isValid
        else { throw QuickTerminalShortcutError.invalidShortcut }

        var eventType = EventTypeSpec(
            eventClass: OSType(kEventClassKeyboard),
            eventKind: UInt32(kEventHotKeyPressed)
        )
        var installedHandler: EventHandlerRef?
        let handlerStatus = InstallEventHandler(
            GetApplicationEventTarget(),
            quickTerminalCarbonEventHandler,
            1,
            &eventType,
            Unmanaged.passUnretained(self).toOpaque(),
            &installedHandler
        )
        guard handlerStatus == noErr else {
            throw QuickTerminalShortcutError.carbonEventHandlerInstallationFailed(handlerStatus)
        }

        var registeredHotKey: EventHotKeyRef?
        let hotKeyID = EventHotKeyID(signature: Self.signature, id: identifier)
        let registrationStatus = RegisterEventHotKey(
            UInt32(virtualKeyCode),
            Self.carbonModifiers(for: combo.nsModifierFlags),
            hotKeyID,
            GetApplicationEventTarget(),
            0,
            &registeredHotKey
        )
        guard registrationStatus == noErr else {
            if let installedHandler {
                RemoveEventHandler(installedHandler)
            }
            throw QuickTerminalShortcutError.carbonHotKeyRegistrationFailed(registrationStatus)
        }

        eventHandler = installedHandler
        hotKey = registeredHotKey
        self.trigger = trigger
    }

    func stop() {
        if let hotKey {
            UnregisterEventHotKey(hotKey)
        }
        if let eventHandler {
            RemoveEventHandler(eventHandler)
        }
        hotKey = nil
        eventHandler = nil
        trigger = nil
    }

    func handle(signature: OSType, identifier: UInt32) -> OSStatus {
        guard signature == Self.signature,
              identifier == self.identifier
        else { return OSStatus(eventNotHandledErr) }
        trigger?()
        return noErr
    }

    static func carbonModifiers(for flags: NSEvent.ModifierFlags) -> UInt32 {
        var modifiers: UInt32 = 0
        if flags.contains(.command) {
            modifiers |= UInt32(cmdKey)
        }
        if flags.contains(.shift) {
            modifiers |= UInt32(shiftKey)
        }
        if flags.contains(.control) {
            modifiers |= UInt32(controlKey)
        }
        if flags.contains(.option) {
            modifiers |= UInt32(optionKey)
        }
        return modifiers
    }
}
