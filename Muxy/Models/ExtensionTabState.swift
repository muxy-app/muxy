import Foundation

@MainActor
@Observable
final class ExtensionTabState: Identifiable {
    let id = UUID()
    let extensionID: String
    let tabTypeID: String
    let projectPath: String
    var data: ExtensionJSON?

    var customTitle: String?
    var defaultTitle: String
    var customIcon: ExtensionIcon?

    var displayTitle: String {
        customTitle ?? defaultTitle
    }

    init(
        extensionID: String,
        tabTypeID: String,
        projectPath: String,
        defaultTitle: String,
        data: ExtensionJSON? = nil
    ) {
        self.extensionID = extensionID
        self.tabTypeID = tabTypeID
        self.projectPath = projectPath
        self.defaultTitle = defaultTitle
        self.data = data
    }
}
