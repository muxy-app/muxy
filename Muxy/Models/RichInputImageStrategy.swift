import Foundation

enum RichInputImageStrategy: String, Codable, CaseIterable, Identifiable {
    case clipboard
    case inlinePath

    var id: String { rawValue }

    var displayName: String {
        switch self {
        case .clipboard: "Clipboard Paste"
        case .inlinePath: "Inline File Path"
        }
    }

    var description: String {
        switch self {
        case .clipboard:
            "Paste image data via the system clipboard. Works with every TUI but uses brief delays between images."
        case .inlinePath:
            "Send image file paths in the bracketed-paste stream. Fully ordered with no delays. "
                + "Requires the receiving TUI to interpret pasted paths as image attachments."
        }
    }
}
