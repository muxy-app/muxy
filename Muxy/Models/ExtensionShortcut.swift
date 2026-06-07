import Foundation

struct ExtensionShortcut: Codable, Identifiable, Equatable {
    let extensionID: String
    let commandID: String
    var combo: KeyCombo

    var id: String { "\(extensionID):\(commandID)" }
}
