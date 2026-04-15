import Foundation

enum QuickSelectKeyboardLayout: String, CaseIterable, Identifiable {
    case qwerty
    case colemak
    case colemakDH
    case dvorak
    case workman
    case custom

    var id: String { rawValue }

    var displayName: String {
        switch self {
        case .qwerty:
            "QWERTY"
        case .colemak:
            "Colemak"
        case .colemakDH:
            "Colemak-DH"
        case .dvorak:
            "Dvorak"
        case .workman:
            "Workman"
        case .custom:
            "Custom"
        }
    }

    var defaultLabels: String {
        switch self {
        case .qwerty:
            "asdfghjkl"
        case .colemak:
            "arstdhneio"
        case .colemakDH:
            "arstgmneio"
        case .dvorak:
            "aoeuidhtns"
        case .workman:
            "ashtgyneoi"
        case .custom:
            ""
        }
    }
}

enum TerminalSettings {
    static let quickSelectLayoutKey = "muxy.terminal.quickSelectLayout"
    static let quickSelectCustomLabelsKey = "muxy.terminal.quickSelectCustomLabels"

    static var quickSelectLabels: [String] {
        let rawLayout = UserDefaults.standard.string(forKey: quickSelectLayoutKey)
        let layout = rawLayout.flatMap(QuickSelectKeyboardLayout.init(rawValue:)) ?? .qwerty
        let rawLabels = switch layout {
        case .qwerty,
             .colemak,
             .colemakDH,
             .dvorak,
             .workman:
            layout.defaultLabels
        case .custom:
            UserDefaults.standard.string(forKey: quickSelectCustomLabelsKey) ?? layout.defaultLabels
        }
        return quickSelectLabels(for: rawLabels)
    }

    static func sanitizeQuickSelectLabelString(_ value: String) -> String {
        sanitizeQuickSelectLabels(value).joined()
    }

    static func quickSelectLabels(for value: String) -> [String] {
        let labels = sanitizeQuickSelectLabels(value)
        guard !labels.isEmpty else {
            return sanitizeQuickSelectLabels(QuickSelectKeyboardLayout.qwerty.defaultLabels)
        }
        return labels
    }

    private static func sanitizeQuickSelectLabels(_ value: String) -> [String] {
        var seen = Set<Character>()
        var result: [String] = []
        for character in value.lowercased() where character.isLetter {
            guard seen.insert(character).inserted else { continue }
            result.append(String(character))
        }
        return result
    }
}
