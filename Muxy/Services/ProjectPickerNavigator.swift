import Foundation

struct ProjectPickerNavigator: Equatable {
    static let parentDirectoryRow = ProjectPickerPathSemantics.parentDirectoryRow

    let input: String
    let homeDirectory: String

    private var semantics: ProjectPickerPathSemantics {
        ProjectPickerPathSemantics(input: input, homeDirectory: homeDirectory)
    }

    var directoryPath: String {
        semantics.directoryPath
    }

    var leafFilter: String {
        semantics.leafFilter
    }

    var confirmPath: String {
        semantics.confirmPath
    }

    var standardizedConfirmPath: String {
        semantics.standardizedConfirmPath
    }

    var parentDisplayPath: String {
        semantics.parentDisplayPath
    }

    func directoryRows(from directoryNames: [String]) -> [String] {
        semantics.directoryRows(from: directoryNames)
    }

    func completedPath(highlightedRow: String) -> String {
        semantics.completedPath(highlightedRow: highlightedRow)
    }

    func ghostText(highlightedRow: String?) -> String {
        semantics.ghostText(highlightedRow: highlightedRow)
    }
}
