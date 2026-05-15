import Foundation

enum ProjectPickerTypedPathState: Equatable {
    case missing
    case directory
    case notDirectory
}

struct ProjectPickerNavigator: Equatable {
    static let parentDirectoryRow = ".."

    let input: String
    let homeDirectory: String

    var directoryPath: String {
        let trimmedInput = input.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmedInput.isEmpty { return "/" }
        if trimmedInput == "~" { return Self.standardizedPath(homeDirectory) }
        let expanded = expandedInput
        guard !expanded.hasSuffix("/") else {
            return Self.standardizedPath(expanded)
        }
        let url = URL(fileURLWithPath: expanded)
        return Self.standardizedPath(url.deletingLastPathComponent().path)
    }

    var leafFilter: String {
        let trimmedInput = input.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmedInput.isEmpty || trimmedInput == "~" || trimmedInput.hasSuffix("/") { return "" }
        return URL(fileURLWithPath: trimmedInput).lastPathComponent
    }

    var confirmPath: String {
        expandedInput
    }

    var standardizedConfirmPath: String {
        Self.standardizedPath(confirmPath)
    }

    var parentDisplayPath: String {
        let currentDirectory = directoryPath
        guard currentDirectory != "/" else { return "/" }
        let parent = Self.standardizedPath(URL(fileURLWithPath: currentDirectory).deletingLastPathComponent().path)
        guard parent != homeDirectory else { return "~/" }
        guard parent.hasPrefix(homeDirectory + "/") else { return parent == "/" ? "/" : parent + "/" }
        return "~" + parent.dropFirst(homeDirectory.count) + "/"
    }

    var directoryReadFailureRows: [String] {
        directoryPath == "/" ? [] : [Self.parentDirectoryRow]
    }

    func directoryRows(from directoryNames: [String]) -> [String] {
        let filter = leafFilter
        let showsDotfiles = filter.hasPrefix(".")
        let rows = directoryNames
            .filter { showsDotfiles || !$0.hasPrefix(".") }
            .filter { filter.isEmpty || $0.localizedCaseInsensitiveContains(filter) }
            .sorted { $0.localizedStandardCompare($1) == .orderedAscending }
        guard directoryPath != "/" else { return rows }
        return [Self.parentDirectoryRow] + rows
    }

    func completedPath(highlightedRow: String) -> String {
        displayDirectoryPrefix + highlightedRow + "/"
    }

    func ghostText(highlightedRow: String?) -> String {
        guard let highlightedRow, !isParentDirectoryRow(highlightedRow) else { return "" }
        let completedPath = completedPath(highlightedRow: highlightedRow)
        if completedPath.hasPrefix(input) {
            return String(completedPath.dropFirst(input.count))
        }
        let trimmedInput = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedInput.contains("/"), !trimmedInput.hasPrefix("~") else { return "" }
        guard highlightedRow.localizedCaseInsensitiveCompare(trimmedInput) != .orderedSame else { return "/" }
        guard highlightedRow.lowercased().hasPrefix(trimmedInput.lowercased()) else { return "" }
        return String(highlightedRow.dropFirst(trimmedInput.count)) + "/"
    }

    func isParentDirectoryRow(_ row: String) -> Bool {
        row == Self.parentDirectoryRow
    }

    static func typedPathState(path: String, fileManager: FileManager = .default) -> ProjectPickerTypedPathState {
        let standardizedPath = standardizedPath(path)
        var isDirectory = ObjCBool(false)
        guard fileManager.fileExists(atPath: standardizedPath, isDirectory: &isDirectory) else {
            return .missing
        }
        return isDirectory.boolValue ? .directory : .notDirectory
    }

    static func defaultLocationStatus(
        path: String,
        fileManager: FileManager = .default
    ) -> ProjectPickerDefaultLocationStatus {
        let standardizedPath = standardizedPath(path)
        var isDirectory = ObjCBool(false)
        guard fileManager.fileExists(atPath: standardizedPath, isDirectory: &isDirectory) else { return .missing }
        guard isDirectory.boolValue else { return .notDirectory }
        guard fileManager.isReadableFile(atPath: standardizedPath) else { return .unreadable }
        return .ready
    }

    static func expandedPath(_ path: String, homeDirectory: String) -> String {
        let trimmedPath = path.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmedPath == "~" { return homeDirectory }
        if trimmedPath.hasPrefix("~/") {
            return homeDirectory + trimmedPath.dropFirst()
        }
        return trimmedPath
    }

    static func standardizedPath(_ path: String) -> String {
        URL(fileURLWithPath: path).standardizedFileURL.path
    }

    static func abbreviatedDirectoryDisplayPath(_ path: String, homeDirectory: String) -> String {
        let standardizedPath = standardizedPath(path)
        let displayPath: String = if standardizedPath == homeDirectory {
            "~"
        } else if standardizedPath.hasPrefix(homeDirectory + "/") {
            "~" + standardizedPath.dropFirst(homeDirectory.count)
        } else {
            standardizedPath
        }
        return displayPath.hasSuffix("/") ? displayPath : displayPath + "/"
    }

    private var displayDirectoryPrefix: String {
        let trimmedInput = input.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmedInput.hasPrefix("~"), directoryPath == homeDirectory { return "~/" }
        if trimmedInput.hasPrefix("~"), directoryPath.hasPrefix(homeDirectory + "/") {
            return "~" + directoryPath.dropFirst(homeDirectory.count) + "/"
        }
        return directoryPath == "/" ? "/" : directoryPath + "/"
    }

    private var expandedInput: String {
        let trimmedInput = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedInput.isEmpty else { return "/" }
        let expandedPath = Self.expandedPath(trimmedInput, homeDirectory: homeDirectory)
        guard expandedPath.hasPrefix("/") else { return "/" + expandedPath }
        return expandedPath
    }
}
