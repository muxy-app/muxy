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

struct ProjectPickerDefaultLocationState: Equatable {
    let path: String
    let displayPath: String
    let usesAppDefault: Bool
    let status: ProjectPickerDefaultLocationStatus
    let chooserInitialPath: String

    var isReady: Bool {
        status == .ready
    }

    var warning: String? {
        status.warning
    }
}

enum ProjectPickerDefaultLocation {
    private static let storageKey = "muxy.projectPicker.defaultDirectory"

    static var path: String { path(defaults: .standard) }
    static var displayPath: String { displayPath(defaults: .standard) }
    static var usesAppDefault: Bool { usesAppDefault(defaults: .standard) }
    static var status: ProjectPickerDefaultLocationStatus { status(defaults: .standard) }
    static var state: ProjectPickerDefaultLocationState { state(defaults: .standard) }

    static func state(
        defaults: UserDefaults,
        pathService: ProjectPickerPathService = ProjectPickerPathService()
    ) -> ProjectPickerDefaultLocationState {
        let path = path(defaults: defaults, pathService: pathService)
        let status = pathService.defaultLocationStatus(path: path)
        return ProjectPickerDefaultLocationState(
            path: path,
            displayPath: pathService.abbreviatedDirectoryDisplayPath(path),
            usesAppDefault: usesAppDefault(defaults: defaults),
            status: status,
            chooserInitialPath: status == .ready ? path : pathService.homeDirectory
        )
    }

    static func path(
        defaults: UserDefaults,
        pathService: ProjectPickerPathService = ProjectPickerPathService()
    ) -> String {
        pathService.expandedPath(storedCustomPath(defaults: defaults) ?? pathService.homeDirectory)
    }

    static func displayPath(defaults: UserDefaults) -> String {
        let pathService = ProjectPickerPathService()
        return pathService.abbreviatedDirectoryDisplayPath(path(defaults: defaults, pathService: pathService))
    }

    static func displayPath(storedCustomPath: String) -> String {
        let pathService = ProjectPickerPathService()
        return pathService.abbreviatedDirectoryDisplayPath(path(
            storedCustomPath: normalizedCustomPath(storedCustomPath),
            pathService: pathService
        ))
    }

    static func usesAppDefault(defaults: UserDefaults) -> Bool {
        storedCustomPath(defaults: defaults) == nil
    }

    static func usesAppDefault(storedCustomPath: String) -> Bool {
        normalizedCustomPath(storedCustomPath) == nil
    }

    static func status(
        defaults: UserDefaults,
        pathService: ProjectPickerPathService = ProjectPickerPathService()
    ) -> ProjectPickerDefaultLocationStatus {
        pathService.defaultLocationStatus(path: path(defaults: defaults, pathService: pathService))
    }

    static func chooserInitialPath(defaults: UserDefaults = .standard) -> String {
        state(defaults: defaults).chooserInitialPath
    }

    static func setCustomPath(_ path: String, defaults: UserDefaults = .standard) {
        guard let path = normalizedCustomPath(path) else {
            resetToAppDefault(defaults: defaults)
            return
        }
        defaults.set(path, forKey: storageKey)
    }

    static func setCustomPath(from url: URL, defaults: UserDefaults = .standard) {
        setCustomPath(url.standardizedFileURL.path, defaults: defaults)
    }

    static func resetToAppDefault(defaults: UserDefaults = .standard) {
        defaults.removeObject(forKey: storageKey)
    }

    private static func storedCustomPath(defaults: UserDefaults) -> String? {
        normalizedCustomPath(defaults.string(forKey: storageKey) ?? "")
    }

    private static func normalizedCustomPath(_ path: String) -> String? {
        let trimmedPath = path.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmedPath.isEmpty ? nil : trimmedPath
    }

    private static func path(
        storedCustomPath: String?,
        pathService: ProjectPickerPathService
    ) -> String {
        pathService.expandedPath(storedCustomPath ?? pathService.homeDirectory)
    }
}
