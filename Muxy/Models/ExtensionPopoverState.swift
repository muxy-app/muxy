import Foundation

@MainActor
@Observable
final class ExtensionPopoverState: Identifiable {
    let id = UUID()
    let extensionID: String
    let popoverID: String
    let initialData: ExtensionJSON?
    var width: Double
    var height: Double

    init(extensionID: String, popoverID: String, width: Double, height: Double, initialData: ExtensionJSON? = nil) {
        self.extensionID = extensionID
        self.popoverID = popoverID
        self.width = width
        self.height = height
        self.initialData = initialData
    }
}
