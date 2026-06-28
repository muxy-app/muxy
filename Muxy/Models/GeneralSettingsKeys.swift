enum GeneralSettingsKeys {
    static let autoExpandWorktreesOnProjectSwitch = "muxy.general.autoExpandWorktreesOnProjectSwitch"
    static let defaultWorktreeParentPath = "muxy.general.defaultWorktreeParentPath"
    static let autoCopyTerminalSelection = "muxy.general.autoCopyTerminalSelection"
    static let terminalFileOpenBehavior = "muxy.general.terminalFileOpenBehavior"
}

enum TerminalFileOpenBehavior: String, CaseIterable, Identifiable {
    case externalEditor = "Open in external editor"
    case inAppOpener = "Open with in-app opener only"

    static let defaultBehavior: TerminalFileOpenBehavior = .externalEditor

    var id: String { rawValue }

    var allowsExternalEditorFallback: Bool {
        self == .externalEditor
    }

    static func resolve(from storedValue: String?) -> TerminalFileOpenBehavior {
        storedValue.flatMap(TerminalFileOpenBehavior.init(rawValue:)) ?? defaultBehavior
    }
}
