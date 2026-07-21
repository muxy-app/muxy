import AppKit
import Testing

@testable import Muxy

@Suite("NotchTerminalShortcut")
struct NotchTerminalShortcutTests {
    @Test("default is double Shift")
    func defaultShortcut() {
        #expect(NotchTerminalShortcut.default == .doubleShift)
        #expect(NotchTerminalShortcut.default.displayString == "Double Shift")
        #expect(NotchTerminalShortcut.default.isValid)
    }

    @Test("key combo exposes display and combo")
    func keyComboValues() {
        let combo = KeyCombo(key: "space", command: true)
        let shortcut = NotchTerminalShortcut.keyCombo(combo, virtualKeyCode: 49)

        #expect(shortcut.displayString == "⌘Space")
        #expect(shortcut.keyCombo == combo)
        #expect(shortcut.virtualKeyCode == 49)
        #expect(shortcut.isValid)
    }

    @Test("key combo requires a modifier and supported key")
    func keyComboValidation() {
        #expect(!NotchTerminalShortcut.keyCombo(KeyCombo(key: "space", modifiers: 0), virtualKeyCode: 49).isValid)
        #expect(!NotchTerminalShortcut.keyCombo(KeyCombo(key: "a", shift: true), virtualKeyCode: 0).isValid)
        #expect(!NotchTerminalShortcut.keyCombo(KeyCombo(key: "missing", command: true), virtualKeyCode: 49).isValid)
        #expect(!NotchTerminalShortcut.keyCombo(KeyCombo(key: "", command: true), virtualKeyCode: 49).isValid)
        #expect(!NotchTerminalShortcut.keyCombo(KeyCombo(key: "a", command: true), virtualKeyCode: 128).isValid)
        #expect(NotchTerminalShortcut.keyCombo(
            KeyCombo(key: "!", command: true, shift: true),
            virtualKeyCode: 18
        ).isValid)
    }

    @Test("Codable round-trip preserves both shortcut kinds", arguments: [
        NotchTerminalShortcut.doubleShift,
        NotchTerminalShortcut.keyCombo(KeyCombo(key: "space", control: true), virtualKeyCode: 49),
    ])
    func codableRoundTrip(shortcut: NotchTerminalShortcut) throws {
        let data = try JSONEncoder().encode(shortcut)
        let decoded = try JSONDecoder().decode(NotchTerminalShortcut.self, from: data)

        #expect(decoded == shortcut)
    }

    @Test("legacy key combo persistence derives a virtual key code")
    func legacyPersistenceMigration() throws {
        let data = Data(#"{"type":"keyCombo","keyCombo":{"key":"space","modifiers":1048576}}"#.utf8)

        let decoded = try JSONDecoder().decode(NotchTerminalShortcut.self, from: data)

        #expect(decoded == .keyCombo(KeyCombo(key: "space", command: true), virtualKeyCode: 49))
    }

    @Test("explicit virtual key code preserves international and keypad identity")
    func explicitVirtualKeyCodeIdentity() {
        let international = NotchTerminalShortcut.keyCombo(
            KeyCombo(key: "q", command: true),
            virtualKeyCode: 0
        )
        let keypad = NotchTerminalShortcut.keyCombo(
            KeyCombo(key: "1", command: true),
            virtualKeyCode: 83
        )

        #expect(international.virtualKeyCode == 0)
        #expect(keypad.virtualKeyCode == 83)
        #expect(international.isValid)
        #expect(keypad.isValid)
    }
}
