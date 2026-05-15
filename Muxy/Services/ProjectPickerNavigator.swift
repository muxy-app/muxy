import Foundation

struct ProjectPickerNavigator {
    static let parentDirectoryRow = ".."

    let input: String
    let homeDirectory: String

    var directoryPath: String {
        let expanded = expandedInput
        guard !expanded.hasSuffix("/") else {
            return standardizedDirectory(expanded)
        }
        let url = URL(fileURLWithPath: expanded)
        return standardizedDirectory(url.deletingLastPathComponent().path)
    }

    var leafFilter: String {
        guard !input.hasSuffix("/") else { return "" }
        return URL(fileURLWithPath: input).lastPathComponent
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
        guard !input.hasSuffix("/") else { return input }
        guard let slashIndex = input.lastIndex(of: "/") else { return "" }
        return String(input[...slashIndex])
    }

    private var expandedInput: String {
        let trimmedInput = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedInput.isEmpty else { return "/" }
        if trimmedInput == "~" { return homeDirectory }
        if trimmedInput.hasPrefix("~/") {
            return homeDirectory + String(trimmedInput.dropFirst())
        }
        return trimmedInput
    }

    private func standardizedDirectory(_ path: String) -> String {
        URL(fileURLWithPath: path).standardizedFileURL.path
    }
}
