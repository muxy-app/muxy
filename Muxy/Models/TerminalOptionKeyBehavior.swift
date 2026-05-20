enum TerminalOptionKeyBehavior: String, CaseIterable, Identifiable {
    case automatic
    case optionAsAlt
    case macOSCharacters
    case left
    case right

    static let ghosttyConfigKey = "macos-option-as-alt"

    var id: Self { self }

    var title: String {
        switch self {
        case .automatic:
            "Automatic"
        case .optionAsAlt:
            "Option as Alt"
        case .macOSCharacters:
            "macOS characters"
        case .left:
            "Left Option"
        case .right:
            "Right Option"
        }
    }

    var ghosttyConfigValue: String? {
        switch self {
        case .automatic:
            nil
        case .optionAsAlt:
            "true"
        case .macOSCharacters:
            "false"
        case .left:
            "left"
        case .right:
            "right"
        }
    }

    init(ghosttyConfigValue: String?) {
        switch ghosttyConfigValue?.lowercased() {
        case "true":
            self = .optionAsAlt
        case "false":
            self = .macOSCharacters
        case "left":
            self = .left
        case "right":
            self = .right
        default:
            self = .automatic
        }
    }
}
