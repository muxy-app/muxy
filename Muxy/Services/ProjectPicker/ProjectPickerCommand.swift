import Foundation

enum ProjectPickerCommand: Hashable {
    case moveHighlightUp
    case moveHighlightDown
    case openHighlighted
    case confirmTypedPath
    case goBack
    case dismiss
    case completeHighlighted

    static var handledIntents: Set<ProjectPickerCommand> {
        Set(allCases)
    }

    static func footerShortcuts(actionTitle: String) -> [ProjectPickerFooterShortcut] {
        [
            ProjectPickerFooterShortcut(intents: [.moveHighlightUp, .moveHighlightDown], keycap: .navigate, label: "Navigate"),
            ProjectPickerFooterShortcut(intents: [.completeHighlighted], keycap: .tab, label: "Autocomplete"),
            ProjectPickerFooterShortcut(intents: [.openHighlighted], keycap: .returnKey, label: "Open"),
            ProjectPickerFooterShortcut(intents: [.confirmTypedPath], keycap: .commandReturn, label: actionTitle),
            ProjectPickerFooterShortcut(intents: [.goBack], keycap: .optionDelete, label: "Go back"),
            ProjectPickerFooterShortcut(intents: [.dismiss], keycap: .escape, label: "Close"),
        ]
    }
}

extension ProjectPickerCommand: CaseIterable {}

struct ProjectPickerFooterShortcut: Hashable {
    let intents: [ProjectPickerCommand]
    let keycap: ProjectPickerShortcutKeycap
    let label: String

    static func ordered(actionTitle: String) -> [ProjectPickerFooterShortcut] {
        ProjectPickerCommand.footerShortcuts(actionTitle: actionTitle)
    }
}

struct ProjectPickerShortcutKeycap: Hashable {
    let parts: [ProjectPickerShortcutKeycapPart]

    static let navigate = ProjectPickerShortcutKeycap(parts: [.symbol("arrow.up"), .symbol("arrow.down")])
    static let tab = ProjectPickerShortcutKeycap(parts: [.text("Tab")])
    static let returnKey = ProjectPickerShortcutKeycap(parts: [.symbol("return")])
    static let commandReturn = ProjectPickerShortcutKeycap(parts: [.symbol("command"), .symbol("return")])
    static let escape = ProjectPickerShortcutKeycap(parts: [.text("Esc")])
    static let optionDelete = ProjectPickerShortcutKeycap(parts: [.symbol("option"), .symbol("delete.left")])
}

enum ProjectPickerShortcutKeycapPart: Hashable {
    case symbol(String)
    case text(String)
}
