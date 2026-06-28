import Foundation

struct ProjectPickerNavigator: Equatable {
    static let parentDirectoryRow = ProjectPickerPathService.parentDirectoryRow

    let pathState: ProjectPickerPathState

    var input: String { pathState.input }
    var homeDirectory: String { pathState.homeDirectory }

    var directoryPath: String {
        pathState.directoryPath
    }

    var leafFilter: String {
        pathState.leafFilter
    }

    var confirmPath: String {
        pathState.confirmPath
    }

    var standardizedConfirmPath: String {
        pathState.standardizedConfirmPath
    }

    var parentDisplayPath: String {
        pathState.parentDisplayPath
    }

    var directoryReadFailureRows: [String] {
        pathState.directoryReadFailureRows
    }

    init(input: String, homeDirectory: String) {
        pathState = ProjectPickerPathService(homeDirectory: homeDirectory).state(for: input)
    }

    init(pathState: ProjectPickerPathState) {
        self.pathState = pathState
    }

    func directoryRows(from directoryNames: [String]) -> [String] {
        pathState.directoryRows(from: directoryNames)
    }

    func completedPath(highlightedRow: String) -> String {
        pathState.completionDisplayPrefix + highlightedRow + "/"
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
}
