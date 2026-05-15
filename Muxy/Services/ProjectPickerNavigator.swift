import Foundation

struct ProjectPickerNavigator {
    static let parentDirectoryRow = ".."

    let input: String
    let homeDirectory: String

    var directoryPath: String {
        let trimmedInput = input.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmedInput.isEmpty { return "/" }
        if trimmedInput == "~" { return standardizedDirectory(homeDirectory) }
        let expanded = expandedInput
        guard !expanded.hasSuffix("/") else {
            return standardizedDirectory(expanded)
        }
        let url = URL(fileURLWithPath: expanded)
        return standardizedDirectory(url.deletingLastPathComponent().path)
    }

    var leafFilter: String {
        let trimmedInput = input.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmedInput.isEmpty || trimmedInput == "~" || trimmedInput.hasSuffix("/") { return "" }
        return URL(fileURLWithPath: trimmedInput).lastPathComponent
    }

    var confirmPath: String {
        expandedInput
    }

    var parentDisplayPath: String {
        let currentDirectory = directoryPath
        guard currentDirectory != "/" else { return "/" }
        let parent = URL(fileURLWithPath: currentDirectory).deletingLastPathComponent().standardizedFileURL.path
        guard parent != homeDirectory else { return "~/" }
        guard parent.hasPrefix(homeDirectory + "/") else { return parent == "/" ? "/" : parent + "/" }
        return "~" + parent.dropFirst(homeDirectory.count) + "/"
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
        let expandedPath = PathExpansion.expandTilde(trimmedInput, homeDirectory: homeDirectory)
        guard expandedPath.hasPrefix("/") else { return "/" + expandedPath }
        return expandedPath
    }

    private func standardizedDirectory(_ path: String) -> String {
        URL(fileURLWithPath: path).standardizedFileURL.path
    }
}
