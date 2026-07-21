import AppKit

enum NotchTerminalShortcut: Codable, Equatable {
    case doubleShift
    case keyCombo(KeyCombo, virtualKeyCode: UInt16)

    static let `default` = NotchTerminalShortcut.doubleShift

    private enum CodingKeys: String, CodingKey {
        case type
        case keyCombo
        case virtualKeyCode
    }

    private enum ShortcutType: String, Codable {
        case doubleShift
        case keyCombo
    }

    var displayString: String {
        switch self {
        case .doubleShift: "Double Shift"
        case let .keyCombo(combo, _): combo.displayString
        }
    }

    var keyCombo: KeyCombo? {
        guard case let .keyCombo(combo, _) = self else { return nil }
        return combo
    }

    var virtualKeyCode: UInt16? {
        guard case let .keyCombo(_, virtualKeyCode) = self else { return nil }
        return virtualKeyCode
    }

    var isValid: Bool {
        switch self {
        case .doubleShift: return true
        case let .keyCombo(combo, virtualKeyCode):
            let conventionalModifiers: NSEvent.ModifierFlags = [.command, .control, .option]
            return combo.isAssigned
                && !combo.nsModifierFlags.isDisjoint(with: conventionalModifiers)
                && (combo.key.count == 1 || KeyCombo.keyCode(for: combo.key) != nil)
                && virtualKeyCode <= 127
        }
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(ShortcutType.self, forKey: .type) {
        case .doubleShift:
            self = .doubleShift
        case .keyCombo:
            let combo = try container.decode(KeyCombo.self, forKey: .keyCombo)
            guard let virtualKeyCode = try container.decodeIfPresent(UInt16.self, forKey: .virtualKeyCode)
                ?? KeyCombo.keyCode(for: combo.key)
            else {
                throw DecodingError.dataCorruptedError(
                    forKey: .virtualKeyCode,
                    in: container,
                    debugDescription: "The shortcut has no supported virtual key code."
                )
            }
            self = .keyCombo(combo, virtualKeyCode: virtualKeyCode)
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .doubleShift:
            try container.encode(ShortcutType.doubleShift, forKey: .type)
        case let .keyCombo(combo, virtualKeyCode):
            try container.encode(ShortcutType.keyCombo, forKey: .type)
            try container.encode(combo, forKey: .keyCombo)
            try container.encode(virtualKeyCode, forKey: .virtualKeyCode)
        }
    }
}
