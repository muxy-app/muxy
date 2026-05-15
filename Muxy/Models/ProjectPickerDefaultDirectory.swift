import Foundation

enum ProjectPickerDefaultDirectoryStatus: Equatable {
    case ready
    case missing
    case notDirectory
    case unreadable

    var warning: String? {
        switch self {
        case .ready:
            nil
        case .missing:
            "Default directory no longer exists. Choose another folder or use the app default."
        case .notDirectory:
            "Default directory is not a folder. Choose another folder or use the app default."
        case .unreadable:
            "Default directory can’t be read. Choose another folder, fix permissions, or use the app default."
        }
    }
}

enum ProjectPickerDefaultDirectory {
    static let storageKey = "muxy.projectPicker.defaultDirectory"

    static var path: String { path(defaults: .standard) }
    static var displayPath: String { displayPath(defaults: .standard) }
    static var usesAppDefault: Bool { usesAppDefault(defaults: .standard) }
    static var status: ProjectPickerDefaultDirectoryStatus { status(defaults: .standard) }

    static func path(defaults: UserDefaults) -> String {
        expandedPath(storedCustomPath(defaults: defaults) ?? NSHomeDirectory())
    }

    static func displayPath(defaults: UserDefaults) -> String {
        abbreviatedPath(path(defaults: defaults))
    }

    static func usesAppDefault(defaults: UserDefaults) -> Bool {
        storedCustomPath(defaults: defaults) == nil
    }

    static func status(defaults: UserDefaults) -> ProjectPickerDefaultDirectoryStatus {
        let path = path(defaults: defaults)
        var isDirectory = ObjCBool(false)
        guard FileManager.default.fileExists(atPath: path, isDirectory: &isDirectory) else { return .missing }
        guard isDirectory.boolValue else { return .notDirectory }
        guard FileManager.default.isReadableFile(atPath: path) else { return .unreadable }
        return .ready
    }

    private static func storedCustomPath(defaults: UserDefaults) -> String? {
        guard let path = defaults.string(forKey: storageKey)?.trimmingCharacters(in: .whitespacesAndNewlines), !path.isEmpty else {
            return nil
        }
        return path
    }

    private static func expandedPath(_ path: String) -> String {
        NSString(string: path).expandingTildeInPath
    }

    private static func abbreviatedPath(_ path: String) -> String {
        let standardizedPath = URL(fileURLWithPath: path).standardizedFileURL.path
        let homeDirectory = NSHomeDirectory()
        let displayPath: String = if standardizedPath == homeDirectory {
            "~"
        } else if standardizedPath.hasPrefix(homeDirectory + "/") {
            "~" + standardizedPath.dropFirst(homeDirectory.count)
        } else {
            standardizedPath
        }
        return displayPath.hasSuffix("/") ? displayPath : displayPath + "/"
    }
}
