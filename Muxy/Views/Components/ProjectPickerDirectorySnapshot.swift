import Foundation

struct ProjectPickerDirectorySnapshot {
    let rows: [String]
    let readFailed: Bool

    static func load(navigator: ProjectPickerNavigator) -> ProjectPickerDirectorySnapshot {
        do {
            let urls = try FileManager.default.contentsOfDirectory(
                at: URL(fileURLWithPath: navigator.directoryPath),
                includingPropertiesForKeys: [.isDirectoryKey, .isSymbolicLinkKey],
                options: []
            )
            let names = urls.compactMap { url -> String? in
                guard isDirectoryOrDirectorySymlink(url) else { return nil }
                return url.lastPathComponent
            }
            return ProjectPickerDirectorySnapshot(rows: navigator.directoryRows(from: names), readFailed: false)
        } catch {
            let rows = navigator.directoryPath == "/" ? [] : [ProjectPickerNavigator.parentDirectoryRow]
            return ProjectPickerDirectorySnapshot(rows: rows, readFailed: true)
        }
    }

    private static func isDirectoryOrDirectorySymlink(_ url: URL) -> Bool {
        let values = try? url.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey])
        if values?.isDirectory == true { return true }
        guard values?.isSymbolicLink == true else { return false }
        var isDirectory = ObjCBool(false)
        return FileManager.default.fileExists(atPath: url.path, isDirectory: &isDirectory) && isDirectory.boolValue
    }
}
