import Darwin
import Foundation

enum ProjectPickerDirectoryReadFailureKind {
    case permissionDenied
    case notFound
    case ioFailure
}

struct ProjectPickerDirectoryReadFailure {
    let kind: ProjectPickerDirectoryReadFailureKind
    let error: Error

    init(error: Error) {
        self.error = error
        kind = Self.kind(for: error as NSError)
    }

    private static func kind(for error: NSError) -> ProjectPickerDirectoryReadFailureKind {
        if let underlying = error.userInfo[NSUnderlyingErrorKey] as? NSError {
            return kind(for: underlying)
        }
        guard error.domain == NSPOSIXErrorDomain else { return .ioFailure }
        switch Int32(error.code) {
        case EACCES,
             EPERM:
            return .permissionDenied
        case ENOENT:
            return .notFound
        default:
            return .ioFailure
        }
    }
}

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
        let parent = URL(fileURLWithPath: directoryPath).deletingLastPathComponent().standardizedFileURL.path
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
