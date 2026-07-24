import AppKit
import Testing

@testable import Muxy

@Suite("QuickTerminalShortcut")
@MainActor
struct QuickTerminalShortcutTests {
    @Test("default is unassigned")
    func defaultShortcut() {
        #expect(QuickTerminalShortcut.default == .unassigned)
        #expect(QuickTerminalShortcut.default.displayString == "Unassigned")
        #expect(QuickTerminalShortcut.default.controlLabel == "Set Shortcut")
        #expect(QuickTerminalShortcut.default.isValid)
    }

    @Test("key combo exposes display and combo")
    func keyComboValues() {
        let combo = KeyCombo(key: "space", command: true)
        let shortcut = QuickTerminalShortcut.keyCombo(combo, virtualKeyCode: 49)

        #expect(shortcut.displayString == "⌘Space")
        #expect(shortcut.keyCombo == combo)
        #expect(shortcut.virtualKeyCode == 49)
        #expect(shortcut.isValid)
    }

    @Test("key combo requires a modifier and supported key")
    func keyComboValidation() {
        #expect(!QuickTerminalShortcut.keyCombo(KeyCombo(key: "space", modifiers: 0), virtualKeyCode: 49).isValid)
        #expect(!QuickTerminalShortcut.keyCombo(KeyCombo(key: "a", shift: true), virtualKeyCode: 0).isValid)
        #expect(!QuickTerminalShortcut.keyCombo(KeyCombo(key: "missing", command: true), virtualKeyCode: 49).isValid)
        #expect(!QuickTerminalShortcut.keyCombo(KeyCombo(key: "", command: true), virtualKeyCode: 49).isValid)
        #expect(!QuickTerminalShortcut.keyCombo(KeyCombo(key: "a", command: true), virtualKeyCode: 128).isValid)
        #expect(QuickTerminalShortcut.keyCombo(
            KeyCombo(key: "!", command: true, shift: true),
            virtualKeyCode: 18
        ).isValid)
    }

    @Test("Codable round-trip preserves both shortcut kinds", arguments: [
        QuickTerminalShortcut.unassigned,
        QuickTerminalShortcut.doubleShift,
        QuickTerminalShortcut.keyCombo(KeyCombo(key: "space", control: true), virtualKeyCode: 49),
    ])
    func codableRoundTrip(shortcut: QuickTerminalShortcut) throws {
        let data = try JSONEncoder().encode(shortcut)
        let decoded = try JSONDecoder().decode(QuickTerminalShortcut.self, from: data)

        #expect(decoded == shortcut)
    }

    @Test("legacy key combo persistence derives a virtual key code")
    func legacyPersistenceMigration() throws {
        let data = Data(#"{"type":"keyCombo","keyCombo":{"key":"space","modifiers":1048576}}"#.utf8)

        let decoded = try JSONDecoder().decode(QuickTerminalShortcut.self, from: data)

        #expect(decoded == .keyCombo(KeyCombo(key: "space", command: true), virtualKeyCode: 49))
    }

    @Test("decoded key combos must use canonical keys and modifiers", arguments: [
        (#"{"type":"keyCombo","keyCombo":{"key":"SPACE","modifiers":1048576},"virtualKeyCode":49}"#),
        (#"{"type":"keyCombo","keyCombo":{"key":"space","modifiers":1114112},"virtualKeyCode":49}"#),
    ])
    func decodedShortcutValidation(json: String) throws {
        let shortcut = try JSONDecoder().decode(QuickTerminalShortcut.self, from: Data(json.utf8))

        #expect(!shortcut.isValid)
    }

    @Test("explicit virtual key code preserves international and keypad identity")
    func explicitVirtualKeyCodeIdentity() {
        let international = QuickTerminalShortcut.keyCombo(
            KeyCombo(key: "q", command: true),
            virtualKeyCode: 0
        )
        let keypad = QuickTerminalShortcut.keyCombo(
            KeyCombo(key: "1", command: true),
            virtualKeyCode: 83
        )

        #expect(international.virtualKeyCode == 0)
        #expect(keypad.virtualKeyCode == 83)
        #expect(international.canonicalized(keyResolver: { $0 == 0 ? "q" : nil }) == international)
        #expect(keypad.canonicalized(keyResolver: { $0 == 83 ? "1" : nil }) == keypad)
    }

    @Test("registration identity derives the display key from the virtual key code")
    func registrationIdentityCanonicalization() {
        let shortcut = QuickTerminalShortcut.keyCombo(
            KeyCombo(key: "q", command: true),
            virtualKeyCode: 49
        )
        let expected = QuickTerminalShortcut.keyCombo(
            KeyCombo(key: "space", command: true),
            virtualKeyCode: 49
        )

        let canonicalized = shortcut.canonicalized(keyResolver: { $0 == 49 ? "space" : nil })

        #expect(canonicalized == expected)
        #expect(canonicalized?.keyCombo == KeyCombo(key: "space", command: true))
        #expect(canonicalized?.displayString == "⌘Space")
    }
}
