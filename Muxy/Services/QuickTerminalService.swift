import AppKit
import Carbon
import SwiftUI

@MainActor
final class QuickTerminalService: NSObject {
    static let shared = QuickTerminalService()

    private var panel: QuickTerminalPanel?
    private var hostingView: NSHostingView<QuickTerminalView>?
    private(set) var isVisible = false
    private var carbonHotKeyRef: EventHotKeyRef?
    private var carbonEventHandler: EventHandlerRef?

    private let paneState = TerminalPaneState(projectPath: NSHomeDirectory())

    override private init() {
        super.init()
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(toggle),
            name: .toggleQuickTerminal,
            object: nil
        )
        registerCarbonHotKey()
    }

    @objc
    func toggle() {
        if isVisible {
            hide()
        } else {
            show()
        }
    }

    // MARK: - Carbon hot key

    private func registerCarbonHotKey() {
        let combo = KeyBindingStore.shared.combo(for: .toggleQuickTerminal)
        guard !combo.key.isEmpty else { return }

        guard let keyCode = carbonKeyCode(for: combo) else { return }
        let modifiers = carbonModifiers(for: combo)

        var hotKeyID = EventHotKeyID()
        hotKeyID.signature = OSType(0x4D55_5859) // "MUXY"
        hotKeyID.id = 1

        var spec = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))

        let selfPtr = Unmanaged.passUnretained(self).toOpaque()
        InstallEventHandler(GetApplicationEventTarget(), { _, event, userData -> OSStatus in
            guard let userData else { return OSStatus(eventNotHandledErr) }
            var hotKeyID = EventHotKeyID()
            GetEventParameter(
                event,
                EventParamName(kEventParamDirectObject),
                EventParamType(typeEventHotKeyID),
                nil,
                MemoryLayout<EventHotKeyID>.size,
                nil,
                &hotKeyID
            )
            if hotKeyID.id == 1 {
                let service = Unmanaged<QuickTerminalService>.fromOpaque(userData).takeUnretainedValue()
                DispatchQueue.main.async { service.toggle() }
            }
            return noErr
        }, 1, &spec, selfPtr, &carbonEventHandler)

        RegisterEventHotKey(keyCode, modifiers, hotKeyID, GetApplicationEventTarget(), 0, &carbonHotKeyRef)
    }

    func reregisterHotKey() {
        if let ref = carbonHotKeyRef {
            UnregisterEventHotKey(ref)
            carbonHotKeyRef = nil
        }
        registerCarbonHotKey()
    }

    private func carbonKeyCode(for combo: KeyCombo) -> UInt32? {
        // Map common keys; fall back to ASCII-based lookup
        let keyMap: [String: UInt32] = [
            "`": 50, "a": 0, "b": 11, "c": 8, "d": 2, "e": 14, "f": 3, "g": 5,
            "h": 4, "i": 34, "j": 38, "k": 40, "l": 37, "m": 46, "n": 45, "o": 31,
            "p": 35, "q": 12, "r": 15, "s": 1, "t": 17, "u": 32, "v": 9, "w": 13,
            "x": 7, "y": 16, "z": 6,
            "0": 29, "1": 18, "2": 19, "3": 20, "4": 21, "5": 23,
            "6": 22, "7": 26, "8": 28, "9": 25,
            "-": 27, "=": 24, "[": 33, "]": 30, "\\": 42, ";": 41, "'": 39,
            ",": 43, ".": 47, "/": 44, " ": 49,
            KeyCombo.leftArrowKey: 123, KeyCombo.rightArrowKey: 124,
            KeyCombo.downArrowKey: 125, KeyCombo.upArrowKey: 126,
        ]
        return keyMap[combo.key]
    }

    private func carbonModifiers(for combo: KeyCombo) -> UInt32 {
        var mods: UInt32 = 0
        let flags = combo.nsModifierFlags
        if flags.contains(.command) { mods |= UInt32(cmdKey) }
        if flags.contains(.shift) { mods |= UInt32(shiftKey) }
        if flags.contains(.control) { mods |= UInt32(controlKey) }
        if flags.contains(.option) { mods |= UInt32(optionKey) }
        return mods
    }

    // MARK: - Show / Hide

    private func show() {
        let panel = makePanel()
        self.panel = panel

        guard let screen = NSScreen.main else { return }
        let screenFrame = screen.visibleFrame
        let widthFraction = UserDefaults.standard.double(forKey: MuxySettings.quickTerminalWidthFractionKey)
        let heightFraction = UserDefaults.standard.double(forKey: MuxySettings.quickTerminalHeightFractionKey)
        let wf = widthFraction > 0 ? widthFraction : MuxySettings.defaultQuickTerminalWidthFraction
        let hf = heightFraction > 0 ? heightFraction : MuxySettings.defaultQuickTerminalHeightFraction
        let panelWidth = screenFrame.width * wf
        let panelHeight = screenFrame.height * hf
        let x = screenFrame.minX + (screenFrame.width - panelWidth) / 2
        let y = screenFrame.minY + (screenFrame.height - panelHeight) / 2

        panel.setFrame(NSRect(x: x, y: y, width: panelWidth, height: panelHeight), display: false)

        let view = QuickTerminalView(paneState: paneState)
        let hosting = NSHostingView(rootView: view)
        hosting.frame = panel.contentView?.bounds ?? .zero
        hosting.autoresizingMask = [.width, .height]
        panel.contentView?.addSubview(hosting)
        self.hostingView = hosting

        panel.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)

        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {
            self.hostingView?.window?.makeFirstResponder(
                TerminalViewRegistry.shared.existingView(for: self.paneState.id)
            )
        }

        isVisible = true
    }

    private func hide() {
        guard let panel else { return }
        panel.orderOut(nil)
        cleanup()
        isVisible = false
    }

    private func cleanup() {
        hostingView?.removeFromSuperview()
        hostingView = nil
        panel = nil
    }

    private func makePanel() -> QuickTerminalPanel {
        let panel = QuickTerminalPanel(
            contentRect: .zero,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        panel.isFloatingPanel = true
        panel.level = .floating
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.contentView = NSView()
        return panel
    }
}

final class QuickTerminalPanel: NSPanel {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
}
