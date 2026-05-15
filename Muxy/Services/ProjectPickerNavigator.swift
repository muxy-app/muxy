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
