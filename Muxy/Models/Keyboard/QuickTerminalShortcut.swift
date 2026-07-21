import AppKit

enum QuickTerminalShortcut: Codable, Equatable {
    case unassigned
    case doubleShift
    case keyCombo(KeyCombo, virtualKeyCode: UInt16)

    static let `default` = QuickTerminalShortcut.unassigned

    private enum CodingKeys: String, CodingKey {
        case type
        case keyCombo
        case virtualKeyCode
    }

    private enum ShortcutType: String, Codable {
        case unassigned
        case doubleShift
        case keyCombo
    }

    var displayString: String {
        switch self {
        case .unassigned: "Unassigned"
        case .doubleShift: "Double Shift"
        case let .keyCombo(combo, _): combo.displayString
        }
    }

    var controlLabel: String {
        self == .unassigned ? "Set Shortcut" : displayString
    }

    var keyCombo: KeyCombo? {
        guard case let .keyCombo(combo, _) = self else { return nil }
        return combo
    }

    var virtualKeyCode: UInt16? {
        guard case let .keyCombo(_, virtualKeyCode) = self else { return nil }
        return virtualKeyCode
    }

    @MainActor
    var isValid: Bool {
        canonicalizedForCurrentKeyboardLayout() != nil
    }

    @MainActor
    func canonicalizedForCurrentKeyboardLayout() -> QuickTerminalShortcut? {
        canonicalized(keyResolver: KeyCombo.key(forVirtualKeyCode:))
    }

    func canonicalized(keyResolver: (UInt16) -> String?) -> QuickTerminalShortcut? {
        switch self {
        case .unassigned:
            return .unassigned
        case .doubleShift:
            return .doubleShift
        case let .keyCombo(_, virtualKeyCode):
            guard let identity = registrationIdentity,
                  let combo = identity.keyCombo(keyResolver: keyResolver)
            else { return nil }
            return .keyCombo(combo, virtualKeyCode: virtualKeyCode)
        }
    }

    func hasSameRegistrationIdentity(as other: QuickTerminalShortcut) -> Bool {
        switch (registrationIdentity, other.registrationIdentity) {
        case let (lhs?, rhs?):
            lhs == rhs
        case (nil, nil):
            self == other
        default:
            false
        }
    }

    private var registrationIdentity: QuickTerminalShortcutRegistrationIdentity? {
        guard case let .keyCombo(combo, virtualKeyCode) = self else { return nil }
        let conventionalModifiers: NSEvent.ModifierFlags = [.command, .control, .option]
        guard combo.isAssigned,
              combo.isCanonical,
              !combo.nsModifierFlags.isDisjoint(with: conventionalModifiers),
              combo.key.count == 1 || KeyCombo.keyCode(for: combo.key) != nil,
              virtualKeyCode <= 127
        else { return nil }
        return QuickTerminalShortcutRegistrationIdentity(
            modifiers: combo.modifiers,
            virtualKeyCode: virtualKeyCode
        )
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(ShortcutType.self, forKey: .type) {
        case .unassigned:
            self = .unassigned
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
        case .unassigned:
            try container.encode(ShortcutType.unassigned, forKey: .type)
        case .doubleShift:
            try container.encode(ShortcutType.doubleShift, forKey: .type)
        case let .keyCombo(combo, virtualKeyCode):
            try container.encode(ShortcutType.keyCombo, forKey: .type)
            try container.encode(combo, forKey: .keyCombo)
            try container.encode(virtualKeyCode, forKey: .virtualKeyCode)
        }
    }
}

struct QuickTerminalShortcutRegistrationIdentity: Equatable {
    let modifiers: UInt
    let virtualKeyCode: UInt16

    func keyCombo(keyResolver: (UInt16) -> String?) -> KeyCombo? {
        guard let key = keyResolver(virtualKeyCode) else { return nil }
        return KeyCombo(key: key, modifiers: modifiers)
    }
}
