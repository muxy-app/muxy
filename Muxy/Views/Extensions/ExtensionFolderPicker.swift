import AppKit

enum ExtensionFolderPicker {
    @MainActor
    static func pick(
        title: LocalizedStringResource,
        message: LocalizedStringResource,
        directory: URL? = nil
    ) -> URL? {
        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false
        panel.canCreateDirectories = true
        panel.prompt = L10n.string("Select")
        panel.title = L10n.string(title)
        panel.message = L10n.string(message)
        if let directory {
            panel.directoryURL = directory
        }
        guard panel.runModal() == .OK else { return nil }
        return panel.url
    }
}
