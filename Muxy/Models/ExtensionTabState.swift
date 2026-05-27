import Foundation

@MainActor
@Observable
final class ExtensionTabState: Identifiable {
    let id = UUID()
    let extensionID: String
    let tabTypeID: String
    let projectPath: String
    let initialData: ExtensionJSON?

    var customTitle: String?
    var defaultTitle: String

    var displayTitle: String {
        customTitle ?? defaultTitle
    }

    init(
        extensionID: String,
        tabTypeID: String,
        projectPath: String,
        defaultTitle: String,
        initialData: ExtensionJSON? = nil
    ) {
        self.extensionID = extensionID
        self.tabTypeID = tabTypeID
        self.projectPath = projectPath
        self.defaultTitle = defaultTitle
        self.initialData = initialData
    }
}
