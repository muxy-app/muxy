import Foundation

enum ProjectPickerDefaultLocationStatus: Equatable {
    case ready
    case missing
    case notDirectory
    case unreadable

    var warning: String? {
        switch self {
        case .ready:
            nil
        case .missing:
            "Default location no longer exists. Choose another folder or use the app default."
        case .notDirectory:
            "Default location is not a folder. Choose another folder or use the app default."
        case .unreadable:
            "Default location can’t be read. Choose another folder, fix permissions, or use the app default."
        }
    }
}

enum ProjectPickerDefaultLocation {
    static let storageKey = "muxy.projectPicker.defaultDirectory"

    static var path: String { path(defaults: .standard) }
    static var displayPath: String { displayPath(defaults: .standard) }
    static var usesAppDefault: Bool { usesAppDefault(defaults: .standard) }
    static var status: ProjectPickerDefaultLocationStatus { status(defaults: .standard) }

    static func path(defaults: UserDefaults) -> String {
        expandedPath(storedCustomPath(defaults: defaults) ?? NSHomeDirectory())
    }

    static func displayPath(defaults: UserDefaults) -> String {
        abbreviatedPath(path(defaults: defaults))
    }

    static func displayPath(storedCustomPath: String) -> String {
        abbreviatedPath(path(storedCustomPath: normalizedCustomPath(storedCustomPath)))
    }

    static func usesAppDefault(defaults: UserDefaults) -> Bool {
        storedCustomPath(defaults: defaults) == nil
    }

    static func usesAppDefault(storedCustomPath: String) -> Bool {
        normalizedCustomPath(storedCustomPath) == nil
    }

    static func status(defaults: UserDefaults) -> ProjectPickerDefaultLocationStatus {
        ProjectPickerPathSemantics.defaultLocationStatus(path: path(defaults: defaults))
    }

    static func chooserInitialPath(defaults: UserDefaults = .standard) -> String {
        status(defaults: defaults) == .ready ? path(defaults: defaults) : NSHomeDirectory()
    }

    private static func storedCustomPath(defaults: UserDefaults) -> String? {
        normalizedCustomPath(defaults.string(forKey: storageKey) ?? "")
    }

    private static func normalizedCustomPath(_ path: String) -> String? {
        let trimmedPath = path.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmedPath.isEmpty ? nil : trimmedPath
    }

    private static func path(storedCustomPath: String?) -> String {
        expandedPath(storedCustomPath ?? NSHomeDirectory())
    }

    private static func expandedPath(_ path: String) -> String {
        ProjectPickerPathSemantics.expandedPath(path, homeDirectory: NSHomeDirectory())
    }

    private static func abbreviatedPath(_ path: String) -> String {
        ProjectPickerPathSemantics.abbreviatedDirectoryDisplayPath(path, homeDirectory: NSHomeDirectory())
    }
}
