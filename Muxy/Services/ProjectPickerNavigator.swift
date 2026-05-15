import Foundation

struct ProjectPickerNavigator {
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

    func directoryRows(from directoryNames: [String]) -> [String] {
        let filter = leafFilter
        let showsDotfiles = filter.hasPrefix(".")
        let rows = directoryNames
            .filter { showsDotfiles || !$0.hasPrefix(".") }
            .filter { filter.isEmpty || $0.localizedCaseInsensitiveContains(filter) }
            .sorted { $0.localizedStandardCompare($1) == .orderedAscending }
        guard directoryPath != "/" else { return rows }
        return [".."] + rows
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
        if input == "~" { return homeDirectory }
        if input.hasPrefix("~/") {
            return homeDirectory + String(input.dropFirst())
        }
        return input
    }

    private func standardizedDirectory(_ path: String) -> String {
        URL(fileURLWithPath: path).standardizedFileURL.path
    }
}
