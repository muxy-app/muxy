import Foundation

@MainActor
@Observable
final class ExtensionPanelState: Identifiable {
    let id = UUID()
    let extensionID: String
    let panelID: String
    let initialData: ExtensionJSON?

    init(extensionID: String, panelID: String, initialData: ExtensionJSON? = nil) {
        self.extensionID = extensionID
        self.panelID = panelID
        self.initialData = initialData
    }

    var hostPanelID: String { ExtensionPanelState.hostPanelID(extensionID: extensionID, panelID: panelID) }

    static func hostPanelID(extensionID: String, panelID: String) -> String {
        "ext:\(extensionID):\(panelID)"
    }
}
