import AppKit
import Carbon.HIToolbox
import SwiftUI

enum ShortcutScope: String, Codable, CaseIterable {
    case global
    case mainWindow
}

struct KeyCombo: Codable, Equatable, Hashable {
    static let supportedModifierMask: NSEvent.ModifierFlags = [.command, .shift, .control, .option]
    static let leftArrowKey = "leftarrow"
    static let rightArrowKey = "rightarrow"
    static let upArrowKey = "uparrow"
    static let downArrowKey = "downarrow"
    private static func keyName(for keyCode: UInt16) -> String? {
        switch Int(keyCode) {
        case kVK_ANSI_A: "a"
        case kVK_ANSI_S: "s"
        case kVK_ANSI_D: "d"
        case kVK_ANSI_F: "f"
        case kVK_ANSI_H: "h"
        case kVK_ANSI_G: "g"
        case kVK_ANSI_Z: "z"
        case kVK_ANSI_X: "x"
        case kVK_ANSI_C: "c"
        case kVK_ANSI_V: "v"
        case kVK_ANSI_B: "b"
        case kVK_ANSI_Q: "q"
        case kVK_ANSI_W: "w"
        case kVK_ANSI_E: "e"
        case kVK_ANSI_R: "r"
        case kVK_ANSI_Y: "y"
        case kVK_ANSI_T: "t"
        case kVK_ANSI_1: "1"
        case kVK_ANSI_2: "2"
        case kVK_ANSI_3: "3"
        case kVK_ANSI_4: "4"
        case kVK_ANSI_6: "6"
        case kVK_ANSI_5: "5"
        case kVK_ANSI_Equal: "="
        case kVK_ANSI_9: "9"
        case kVK_ANSI_7: "7"
        case kVK_ANSI_Minus: "-"
        case kVK_ANSI_8: "8"
        case kVK_ANSI_0: "0"
        case kVK_ANSI_RightBracket: "]"
        case kVK_ANSI_O: "o"
        case kVK_ANSI_U: "u"
        case kVK_ANSI_LeftBracket: "["
        case kVK_ANSI_I: "i"
        case kVK_ANSI_P: "p"
        case kVK_ANSI_L: "l"
        case kVK_ANSI_J: "j"
        case kVK_ANSI_Quote: "'"
        case kVK_ANSI_K: "k"
        case kVK_ANSI_Semicolon: ";"
        case kVK_ANSI_Backslash: "\\"
        case kVK_ANSI_Comma: ","
        case kVK_ANSI_Slash: "/"
        case kVK_ANSI_N: "n"
        case kVK_ANSI_M: "m"
        case kVK_ANSI_Period: "."
        case kVK_ANSI_Grave: "`"
        case kVK_ANSI_KeypadDecimal: "."
        case kVK_ANSI_KeypadMultiply: "*"
        case kVK_ANSI_KeypadPlus: "+"
        case kVK_ANSI_KeypadDivide: "/"
        case kVK_ANSI_KeypadMinus: "-"
        case kVK_ANSI_KeypadEquals: "="
        case kVK_ANSI_Keypad0: "0"
        case kVK_ANSI_Keypad1: "1"
        case kVK_ANSI_Keypad2: "2"
        case kVK_ANSI_Keypad3: "3"
        case kVK_ANSI_Keypad4: "4"
        case kVK_ANSI_Keypad5: "5"
        case kVK_ANSI_Keypad6: "6"
        case kVK_ANSI_Keypad7: "7"
        case kVK_ANSI_Keypad8: "8"
        case kVK_ANSI_Keypad9: "9"
        case kVK_LeftArrow: leftArrowKey
        case kVK_RightArrow: rightArrowKey
        case kVK_DownArrow: downArrowKey
        case kVK_UpArrow: upArrowKey
        default: nil
        }
    }

    static func keyCode(for keyName: String) -> UInt16? {
        let name = keyName.lowercased()
        for code in 0 ... 127 where Self.keyName(for: UInt16(code)) == name {
            return UInt16(code)
        }
        return nil
    }

    let key: String
    let modifiers: UInt

    init(key: String, modifiers: UInt) {
        self.key = Self.normalized(key: key)
        self.modifiers = Self.normalized(modifiers: modifiers)
    }

    init(
        key: String, command: Bool = false, shift: Bool = false, control: Bool = false,
        option: Bool = false
    ) {
        self.key = Self.normalized(key: key)
        var flags: UInt = 0
        if command { flags |= NSEvent.ModifierFlags.command.rawValue }
        if shift { flags |= NSEvent.ModifierFlags.shift.rawValue }
        if control { flags |= NSEvent.ModifierFlags.control.rawValue }
        if option { flags |= NSEvent.ModifierFlags.option.rawValue }
        self.modifiers = flags
    }

    var nsModifierFlags: NSEvent.ModifierFlags {
        NSEvent.ModifierFlags(rawValue: modifiers).intersection(Self.supportedModifierMask)
    }

    var swiftUIKeyEquivalent: KeyEquivalent {
        switch key {
        case "[": KeyEquivalent("[")
        case "]": KeyEquivalent("]")
        case ",": KeyEquivalent(",")
        case Self.leftArrowKey: .leftArrow
        case Self.rightArrowKey: .rightArrow
        case Self.upArrowKey: .upArrow
        case Self.downArrowKey: .downArrow
        default: KeyEquivalent(Character(key))
        }
    }

    var swiftUIModifiers: SwiftUI.EventModifiers {
        var result: SwiftUI.EventModifiers = []
        let flags = nsModifierFlags
        if flags.contains(.command) { result.insert(.command) }
        if flags.contains(.shift) { result.insert(.shift) }
        if flags.contains(.control) { result.insert(.control) }
        if flags.contains(.option) { result.insert(.option) }
        return result
    }

    var displayString: String {
        var parts = ""
        let flags = nsModifierFlags
        if flags.contains(.control) { parts += "⌃" }
        if flags.contains(.option) { parts += "⌥" }
        if flags.contains(.shift) { parts += "⇧" }
        if flags.contains(.command) { parts += "⌘" }
        let keyDisplay: String = switch key {
        case Self.leftArrowKey: "←"
        case Self.rightArrowKey: "→"
        case Self.upArrowKey: "↑"
        case Self.downArrowKey: "↓"
        default: key.uppercased()
        }
        parts += keyDisplay
        return parts
    }

    func matches(event: NSEvent) -> Bool {
        let eventFlags = event.modifierFlags.intersection(Self.supportedModifierMask).rawValue
        let eventKey = Self.normalized(key: event.charactersIgnoringModifiers ?? "", keyCode: event.keyCode)
        return eventKey == key && eventFlags == modifiers
    }

    static func normalized(modifiers: UInt) -> UInt {
        NSEvent.ModifierFlags(rawValue: modifiers).intersection(supportedModifierMask).rawValue
    }

    static func scalar(for keyCode: UInt16) -> UnicodeScalar? {
        guard let mappedKey = keyName(for: keyCode),
              mappedKey.unicodeScalars.count == 1
        else { return nil }
        return mappedKey.unicodeScalars.first
    }

    static func normalized(key: String, keyCode: UInt16? = nil) -> String {
        let lowercased = key.lowercased()
        if lowercased == leftArrowKey || lowercased == rightArrowKey || lowercased == upArrowKey || lowercased == downArrowKey {
            return lowercased
        }

        if let scalar = lowercased.unicodeScalars.first, lowercased.unicodeScalars.count == 1 {
            switch Int(scalar.value) {
            case NSLeftArrowFunctionKey: return leftArrowKey
            case NSRightArrowFunctionKey: return rightArrowKey
            case NSUpArrowFunctionKey: return upArrowKey
            case NSDownArrowFunctionKey: return downArrowKey
            default:
                if scalar.isASCII, scalar.value >= 32, scalar.value <= 126 {
                    return lowercased
                }
            }
        }

        if let keyCode, let mappedKey = keyName(for: keyCode) {
            return mappedKey
        }

        return lowercased
    }
}
