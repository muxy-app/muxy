import AppKit
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
    private static let keyCodeMap: [Int: String] = [
        0: "a",
        1: "s",
        2: "d",
        3: "f",
        4: "h",
        5: "g",
        6: "z",
        7: "x",
        8: "c",
        9: "v",
        11: "b",
        12: "q",
        13: "w",
        14: "e",
        15: "r",
        16: "y",
        17: "t",
        18: "1",
        19: "2",
        20: "3",
        21: "4",
        22: "6",
        23: "5",
        24: "=",
        25: "9",
        26: "7",
        27: "-",
        28: "8",
        29: "0",
        30: "]",
        31: "o",
        32: "u",
        33: "[",
        34: "i",
        35: "p",
        37: "l",
        38: "j",
        39: "'",
        40: "k",
        41: ";",
        42: "\\",
        43: ",",
        44: "/",
        45: "n",
        46: "m",
        47: ".",
        50: "`",
        65: ".",
        67: "*",
        69: "+",
        75: "/",
        78: "-",
        81: "=",
        82: "0",
        83: "1",
        84: "2",
        85: "3",
        86: "4",
        87: "5",
        88: "6",
        89: "7",
        91: "8",
        92: "9",
        123: leftArrowKey,
        124: rightArrowKey,
        125: downArrowKey,
        126: upArrowKey,
    ]

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

    var swiftUIModifiers: EventModifiers {
        var result: EventModifiers = []
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
        guard let mappedKey = keyCodeMap[Int(keyCode)],
              mappedKey.unicodeScalars.count == 1
        else { return nil }
        return mappedKey.unicodeScalars.first
    }

    static func normalized(key: String, keyCode: UInt16? = nil) -> String {
        if let keyCode, let mappedKey = keyCodeMap[Int(keyCode)] {
            return mappedKey
        }

        let lowercased = key.lowercased()
        if lowercased == leftArrowKey || lowercased == rightArrowKey || lowercased == upArrowKey || lowercased == downArrowKey {
            return lowercased
        }

        guard let scalar = lowercased.unicodeScalars.first, lowercased.unicodeScalars.count == 1 else {
            return lowercased
        }

        switch Int(scalar.value) {
        case NSLeftArrowFunctionKey: return leftArrowKey
        case NSRightArrowFunctionKey: return rightArrowKey
        case NSUpArrowFunctionKey: return upArrowKey
        case NSDownArrowFunctionKey: return downArrowKey
        default: return lowercased
        }
    }
}
