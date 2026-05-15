import Foundation

enum ProjectPickerCommand: Hashable {
    case navigate
    case moveHighlightUp
    case moveHighlightDown
    case openHighlighted
    case confirmTypedPath
    case goBack
    case dismiss
    case completeHighlighted

    static let footerCommands: [ProjectPickerCommand] = [
        .navigate,
        .completeHighlighted,
        .openHighlighted,
        .confirmTypedPath,
        .goBack,
        .dismiss,
    ]

    var isSessionHandled: Bool {
        !sessionCommands.isEmpty
    }

    var sessionCommands: [ProjectPickerCommand] {
        switch self {
        case .navigate:
            [.moveHighlightUp, .moveHighlightDown]
        case .moveHighlightUp,
             .moveHighlightDown,
             .openHighlighted,
             .confirmTypedPath,
             .goBack,
             .dismiss,
             .completeHighlighted:
            [self]
        }
    }

    func footerShortcut(actionTitle: String) -> ProjectPickerFooterShortcut? {
        switch self {
        case .navigate:
            ProjectPickerFooterShortcut(command: self, keycap: .navigate, label: "Navigate")
        case .completeHighlighted:
            ProjectPickerFooterShortcut(command: self, keycap: .tab, label: "Autocomplete")
        case .openHighlighted:
            ProjectPickerFooterShortcut(command: self, keycap: .returnKey, label: "Open")
        case .confirmTypedPath:
            ProjectPickerFooterShortcut(command: self, keycap: .commandReturn, label: actionTitle)
        case .goBack:
            ProjectPickerFooterShortcut(command: self, keycap: .optionDelete, label: "Go back")
        case .dismiss:
            ProjectPickerFooterShortcut(command: self, keycap: .escape, label: "Close")
        case .moveHighlightUp,
             .moveHighlightDown:
            nil
        }
    }
}
