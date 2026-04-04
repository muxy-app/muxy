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
    private static let keyCodeLeftBracket = 33
    private static let keyCodeRightBracket = 30
    private static let keyCodeLeftArrow = 123
    private static let keyCodeRightArrow = 124
    private static let keyCodeDownArrow = 125
    private static let keyCodeUpArrow = 126

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

    static func normalized(key: String, keyCode: UInt16? = nil) -> String {
        if let keyCode {
            switch Int(keyCode) {
            case keyCodeLeftBracket: return "["
            case keyCodeRightBracket: return "]"
            case keyCodeLeftArrow: return leftArrowKey
            case keyCodeRightArrow: return rightArrowKey
            case keyCodeUpArrow: return upArrowKey
            case keyCodeDownArrow: return downArrowKey
            default: break
            }
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
