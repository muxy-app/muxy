import Foundation

struct ProjectPickerDirectorySnapshot: Equatable {
    let rows: [String]
    let readFailed: Bool

    static func load(
        navigator: ProjectPickerNavigator,
        fileManager: FileManager = .default
    ) -> ProjectPickerDirectorySnapshot {
        do {
            let urls = try fileManager.contentsOfDirectory(
                at: URL(fileURLWithPath: navigator.directoryPath),
                includingPropertiesForKeys: [.isDirectoryKey, .isSymbolicLinkKey],
                options: []
            )
            let names = urls.compactMap { url -> String? in
                guard isDirectoryOrDirectorySymlink(url, fileManager: fileManager) else { return nil }
                return url.lastPathComponent
            }
            return ProjectPickerDirectorySnapshot(rows: navigator.directoryRows(from: names), readFailed: false)
        } catch {
            return ProjectPickerDirectorySnapshot(rows: navigator.directoryReadFailureRows, readFailed: true)
        }
    }

    private static func isDirectoryOrDirectorySymlink(_ url: URL, fileManager: FileManager) -> Bool {
        let values = try? url.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey])
        if values?.isDirectory == true { return true }
        guard values?.isSymbolicLink == true else { return false }
        var isDirectory = ObjCBool(false)
        return fileManager.fileExists(atPath: url.path, isDirectory: &isDirectory) && isDirectory.boolValue
    }
}
