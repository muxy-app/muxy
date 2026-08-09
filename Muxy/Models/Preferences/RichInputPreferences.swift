import Foundation

enum RichInputPresentationMode: String, CaseIterable, Identifiable {
    case panel
    case floating

    var id: String { rawValue }

    var displayName: String {
        switch self {
        case .panel: "Panel"
        case .floating: "Floating"
        }
    }
}

enum RichInputPreferences {
    static let fontSizeKey = "muxy.richInput.fontSize"
    static let defaultFontSize: Double = 13
    static let minFontSize: Double = 9
    static let maxFontSize: Double = 32
    static let fontStep: Double = 1

    static let broadcastKey = "muxy.richInput.broadcast"
    static let defaultBroadcast = false

    static let presentationModeKey = "muxy.richInput.presentationMode"
    static let defaultPresentationMode: RichInputPresentationMode = .panel

    static let positionKey = "muxy.richInput.position"
    static let defaultPosition: PanelPosition = .right
}
