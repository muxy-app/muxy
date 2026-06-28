import AppKit
import Testing

@testable import Muxy

@Suite("KeyCombo")
struct KeyComboTests {
    @Test("init normalizes key to lowercase")
    func initNormalizesKey() {
        let combo = KeyCombo(key: "A", modifiers: 0)
        #expect(combo.key == "a")
    }

    @Test("init normalizes modifiers to strip unsupported bits")
    func initNormalizesModifiers() {
        let capsLock = NSEvent.ModifierFlags.capsLock.rawValue
        let command = NSEvent.ModifierFlags.command.rawValue
        let combo = KeyCombo(key: "a", modifiers: capsLock | command)
        #expect(combo.modifiers == command)
    }

    @Test("convenience init with booleans builds correct bitmask")
    func initWithBooleans() {
        let combo = KeyCombo(key: "t", command: true, shift: true)
        let expected = NSEvent.ModifierFlags.command.rawValue | NSEvent.ModifierFlags.shift.rawValue
        #expect(combo.modifiers == expected)
    }

    @Test("convenience init with no modifiers")
    func initNoModifiers() {
        let combo = KeyCombo(key: "a", command: false)
        #expect(combo.modifiers == 0)
    }

    @Test("displayString shows modifiers in correct order")
    func displayStringModifierOrder() {
        let combo = KeyCombo(key: "a", command: true, shift: true, control: true, option: true)
        #expect(combo.displayString == "⌃⌥⇧⌘A")
    }

    @Test("displayString for arrow keys")
    func displayStringArrowKeys() {
        #expect(KeyCombo(key: "leftarrow", command: true).displayString == "⌘←")
        #expect(KeyCombo(key: "rightarrow", command: true).displayString == "⌘→")
        #expect(KeyCombo(key: "uparrow", command: true).displayString == "⌘↑")
        #expect(KeyCombo(key: "downarrow", command: true).displayString == "⌘↓")
    }

    @Test("displayString for tab key")
    func displayStringTabKey() {
        #expect(KeyCombo(key: "tab", control: true).displayString == "⌃⇥")
        #expect(KeyCombo(key: "tab", shift: true, control: true).displayString == "⌃⇧⇥")
    }

    @Test("displayString for return key")
    func displayStringReturnKey() throws {
        let keyCode = try #require(KeyCombo.keyCode(for: "return"))
        let combo = KeyCombo(key: "return", modifiers: NSEvent.ModifierFlags.command.rawValue)

        #expect(KeyCombo.normalized(key: "", keyCode: keyCode) == "return")
        #expect(KeyCombo(key: "return", command: true).displayString == "⌘↩")
        _ = combo.swiftUIKeyEquivalent
    }

    @Test("displayString for letter key is uppercased")
    func displayStringLetter() {
        let combo = KeyCombo(key: "t", command: true)
        #expect(combo.displayString == "⌘T")
    }

    @Test("displayString for unassigned shortcut")
    func displayStringUnassigned() {
        let combo = KeyCombo(key: "", modifiers: 0)
        #expect(combo.displayString == "Unassigned")
    }

    @Test("normalized key with bracket keyCodes")
    func normalizedBracketKeyCodes() {
        #expect(KeyCombo.normalized(key: "", keyCode: 33) == "[")
        #expect(KeyCombo.normalized(key: "", keyCode: 30) == "]")
    }

    @Test("normalized key with arrow keyCodes")
    func normalizedArrowKeyCodes() {
        #expect(KeyCombo.normalized(key: "", keyCode: 123) == "leftarrow")
        #expect(KeyCombo.normalized(key: "", keyCode: 124) == "rightarrow")
        #expect(KeyCombo.normalized(key: "", keyCode: 125) == "downarrow")
        #expect(KeyCombo.normalized(key: "", keyCode: 126) == "uparrow")
    }

    @Test("normalized key with tab keyCode")
    func normalizedTabKeyCode() {
        #expect(KeyCombo.normalized(key: "", keyCode: 48) == "tab")
    }

    @Test("normalized key with ANSI letter keyCodes")
    func normalizedLetterKeyCodes() {
        #expect(KeyCombo.normalized(key: "\u{043C}", keyCode: 9) == "v")
        #expect(KeyCombo.normalized(key: "\u{0439}", keyCode: 12) == "q")
    }

    @Test("keyCode lookup scans supported keyboard mappings")
    func keyCodeLookupScansSupportedMappings() throws {
        for name in [
            "a", "s", "d", "f", "h", "g", "z", "x", "c", "v", "b", "q", "w", "e", "r", "y", "t",
            "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "=", "-", "]", "o", "u", "[", "i",
            "p", "l", "j", "'", "k", ";", "\\", ",", "/", "n", "m", ".", "`", "*", "+",
            "leftarrow", "rightarrow", "downarrow", "uparrow", "tab", "return",
        ] {
            let code = try #require(KeyCombo.keyCode(for: name))
            #expect(KeyCombo.normalized(key: "", keyCode: code) == name)
        }

        #expect(KeyCombo.keyCode(for: "missing") == nil)
    }

    @Test("SwiftUI modifiers mirror AppKit modifier flags")
    func swiftUIModifiersMirrorAppKitFlags() {
        let combo = KeyCombo(key: "p", command: true, shift: true, control: true, option: true)
        let modifiers = combo.swiftUIModifiers

        #expect(combo.nsModifierFlags.contains(.command))
        #expect(modifiers.contains(.command))
        #expect(modifiers.contains(.shift))
        #expect(modifiers.contains(.control))
        #expect(modifiers.contains(.option))
    }

    @Test("scalar uses ANSI keyCode mapping")
    func scalarFromKeyCode() {
        #expect(KeyCombo.scalar(for: 9)?.value == Unicode.Scalar("v").value)
        #expect(KeyCombo.scalar(for: 43)?.value == Unicode.Scalar(",").value)
    }

    @Test("normalized key with function key scalars")
    func normalizedFunctionKeys() {
        let leftArrowScalar = Unicode.Scalar(NSLeftArrowFunctionKey)!
        #expect(KeyCombo.normalized(key: String(leftArrowScalar)) == "leftarrow")

        let rightArrowScalar = Unicode.Scalar(NSRightArrowFunctionKey)!
        #expect(KeyCombo.normalized(key: String(rightArrowScalar)) == "rightarrow")
    }

    @Test("normalized key lowercases arrow key names")
    func normalizedArrowNames() {
        #expect(KeyCombo.normalized(key: "LeftArrow") == "leftarrow")
        #expect(KeyCombo.normalized(key: "RIGHTARROW") == "rightarrow")
    }

    @Test("Codable round-trip preserves values")
    func codableRoundTrip() throws {
        let original = KeyCombo(key: "t", command: true, shift: true)
        let data = try JSONEncoder().encode(original)
        let decoded = try JSONDecoder().decode(KeyCombo.self, from: data)
        #expect(decoded == original)
    }

    @Test("Equatable for same values")
    func equatable() {
        let a = KeyCombo(key: "a", command: true)
        let b = KeyCombo(key: "a", command: true)
        #expect(a == b)
    }

    @Test("Equatable for different values")
    func notEqual() {
        let a = KeyCombo(key: "a", command: true)
        let b = KeyCombo(key: "b", command: true)
        #expect(a != b)

        let c = KeyCombo(key: "a", command: true)
        let d = KeyCombo(key: "a", shift: true)
        #expect(c != d)
    }

    @Test("Hashable works as dictionary key")
    func hashable() {
        let combo = KeyCombo(key: "t", command: true)
        var dict: [KeyCombo: String] = [:]
        dict[combo] = "test"
        #expect(dict[KeyCombo(key: "t", command: true)] == "test")
    }
}
