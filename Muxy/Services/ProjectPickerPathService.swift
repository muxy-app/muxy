import Foundation

enum ProjectPickerTypedPathState: Equatable {
    case missing
    case directory
    case notDirectory
}

enum ProjectPickerFileSystemDirectoryState: Equatable {
    case missing
    case directory
    case notDirectory
}

enum ProjectPickerFileSystemDirectoryEntry: Equatable {
    case directory(String)
    case directorySymlink(String)
    case file(String)
    case fileSymlink(String)

    var name: String {
        switch self {
        case let .directory(name),
             let .directorySymlink(name),
             let .file(name),
             let .fileSymlink(name):
            name
        }
    }

    var isProjectPickerDirectory: Bool {
        switch self {
        case .directory,
             .directorySymlink:
            true
        case .file,
             .fileSymlink:
            false
        }
    }

    var projectPickerDirectoryItem: ProjectPickerDirectoryItem? {
        switch self {
        case let .directory(name):
            .directory(name)
        case let .directorySymlink(name):
            .directorySymlink(name)
        case .file,
             .fileSymlink:
            nil
        }
    }
}

protocol ProjectPickerFileSystem {
    func directoryState(atPath path: String) -> ProjectPickerFileSystemDirectoryState
    func isReadableFile(atPath path: String) -> Bool
    func contentsOfDirectory(atPath path: String) throws -> [ProjectPickerFileSystemDirectoryEntry]
}

struct FileManagerProjectPickerFileSystem: ProjectPickerFileSystem {
    var fileManager: FileManager = .default

    func directoryState(atPath path: String) -> ProjectPickerFileSystemDirectoryState {
        var isDirectory = ObjCBool(false)
        guard fileManager.fileExists(atPath: path, isDirectory: &isDirectory) else {
            return .missing
        }
        return isDirectory.boolValue ? .directory : .notDirectory
    }

    func isReadableFile(atPath path: String) -> Bool {
        fileManager.isReadableFile(atPath: path)
    }

    func contentsOfDirectory(atPath path: String) throws -> [ProjectPickerFileSystemDirectoryEntry] {
        try fileManager.contentsOfDirectory(
            at: URL(fileURLWithPath: path),
            includingPropertiesForKeys: [.isDirectoryKey, .isSymbolicLinkKey],
            options: []
        )
        .map(directoryEntry)
    }

    private func directoryEntry(for url: URL) -> ProjectPickerFileSystemDirectoryEntry {
        let values = try? url.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey])
        guard values?.isSymbolicLink == true else {
            return values?.isDirectory == true ? .directory(url.lastPathComponent) : .file(url.lastPathComponent)
        }

        var isDirectory = ObjCBool(false)
        let pointsToDirectory = fileManager.fileExists(atPath: url.path, isDirectory: &isDirectory) && isDirectory.boolValue
        return pointsToDirectory ? .directorySymlink(url.lastPathComponent) : .fileSymlink(url.lastPathComponent)
    }
}

struct ProjectPickerPathState: Equatable {
    let input: String
    let homeDirectory: String
    let directoryPath: String
    let leafFilter: String
    let confirmPath: String
    let standardizedConfirmPath: String
    let parentDisplayPath: String
    let completionDisplayPrefix: String

    var directoryReadFailureItems: [ProjectPickerDirectoryItem] {
        directoryPath == "/" ? [] : [.parent]
    }

    var directoryReadFailureRows: [String] {
        directoryReadFailureItems.map(\.name)
    }

    func directoryRows(from directoryNames: [String]) -> [String] {
        directoryItems(from: directoryNames.map(ProjectPickerDirectoryItem.directory)).map(\.name)
    }

    func directoryItems(from directoryItems: [ProjectPickerDirectoryItem]) -> [ProjectPickerDirectoryItem] {
        let showsDotfiles = leafFilter.hasPrefix(".")
        let rows = directoryItems
            .filter { showsDotfiles || !$0.name.hasPrefix(".") }
            .filter { leafFilter.isEmpty || $0.name.localizedCaseInsensitiveContains(leafFilter) }
            .sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
        guard directoryPath != "/" else { return rows }
        return [.parent] + rows
    }
}

struct ProjectPickerPathService {
    static let parentDirectoryRow = ".."

    let homeDirectory: String
    private let fileSystem: any ProjectPickerFileSystem

    init(
        homeDirectory: String = NSHomeDirectory(),
        fileSystem: any ProjectPickerFileSystem = FileManagerProjectPickerFileSystem()
    ) {
        self.homeDirectory = homeDirectory
        self.fileSystem = fileSystem
    }

    func state(for input: String) -> ProjectPickerPathState {
        let trimmedInput = input.trimmingCharacters(in: .whitespacesAndNewlines)
        let confirmPath = confirmPath(for: trimmedInput)
        let directoryPath = directoryPath(for: trimmedInput, expandedInput: confirmPath)
        let leafFilter = leafFilter(for: trimmedInput)
        return ProjectPickerPathState(
            input: input,
            homeDirectory: homeDirectory,
            directoryPath: directoryPath,
            leafFilter: leafFilter,
            confirmPath: confirmPath,
            standardizedConfirmPath: Self.standardizedPath(confirmPath),
            parentDisplayPath: parentDisplayPath(for: directoryPath),
            completionDisplayPrefix: completionDisplayPrefix(for: trimmedInput, directoryPath: directoryPath)
        )
    }

    func typedPathState(path: String) -> ProjectPickerTypedPathState {
        switch fileSystem.directoryState(atPath: Self.standardizedPath(path)) {
        case .missing:
            .missing
        case .directory:
            .directory
        case .notDirectory:
            .notDirectory
        }
    }

    func defaultLocationStatus(path: String) -> ProjectPickerDefaultLocationStatus {
        let standardizedPath = Self.standardizedPath(path)
        switch fileSystem.directoryState(atPath: standardizedPath) {
        case .missing:
            return .missing
        case .notDirectory:
            return .notDirectory
        case .directory:
            return fileSystem.isReadableFile(atPath: standardizedPath) ? .ready : .unreadable
        }
    }

    func directorySnapshot(for pathState: ProjectPickerPathState) -> ProjectPickerDirectorySnapshot {
        do {
            let items = try fileSystem.contentsOfDirectory(atPath: pathState.directoryPath)
                .compactMap(\.projectPickerDirectoryItem)
            return ProjectPickerDirectorySnapshot(rows: pathState.directoryItems(from: items), readFailed: false)
        } catch {
            return ProjectPickerDirectorySnapshot(rows: pathState.directoryReadFailureItems, readFailed: true)
        }
    }

    func expandedPath(_ path: String) -> String {
        let trimmedPath = path.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmedPath == "~" { return homeDirectory }
        if trimmedPath.hasPrefix("~/") {
            return homeDirectory + trimmedPath.dropFirst()
        }
        return trimmedPath
    }

    func abbreviatedDirectoryDisplayPath(_ path: String) -> String {
        let standardizedPath = Self.standardizedPath(path)
        let displayPath: String = if standardizedPath == homeDirectory {
            "~"
        } else if standardizedPath.hasPrefix(homeDirectory + "/") {
            "~" + standardizedPath.dropFirst(homeDirectory.count)
        } else {
            standardizedPath
        }
        return displayPath.hasSuffix("/") ? displayPath : displayPath + "/"
    }

    static func standardizedPath(_ path: String) -> String {
        URL(fileURLWithPath: path).standardizedFileURL.path
    }

    private func confirmPath(for trimmedInput: String) -> String {
        guard !trimmedInput.isEmpty else { return "/" }
        let expandedPath = expandedPath(trimmedInput)
        guard expandedPath.hasPrefix("/") else { return "/" + expandedPath }
        return expandedPath
    }

    private func directoryPath(for trimmedInput: String, expandedInput: String) -> String {
        if trimmedInput.isEmpty { return "/" }
        if trimmedInput == "~" { return Self.standardizedPath(homeDirectory) }
        guard !expandedInput.hasSuffix("/") else {
            return Self.standardizedPath(expandedInput)
        }
        return Self.standardizedPath(URL(fileURLWithPath: expandedInput).deletingLastPathComponent().path)
    }

    private func leafFilter(for trimmedInput: String) -> String {
        if trimmedInput.isEmpty || trimmedInput == "~" || trimmedInput.hasSuffix("/") { return "" }
        return URL(fileURLWithPath: trimmedInput).lastPathComponent
    }

    private func parentDisplayPath(for directoryPath: String) -> String {
        guard directoryPath != "/" else { return "/" }
        let parent = Self.standardizedPath(URL(fileURLWithPath: directoryPath).deletingLastPathComponent().path)
        guard parent != homeDirectory else { return "~/" }
        guard parent.hasPrefix(homeDirectory + "/") else { return parent == "/" ? "/" : parent + "/" }
        return "~" + parent.dropFirst(homeDirectory.count) + "/"
    }

    private func completionDisplayPrefix(for trimmedInput: String, directoryPath: String) -> String {
        if trimmedInput.hasPrefix("~"), directoryPath == homeDirectory { return "~/" }
        if trimmedInput.hasPrefix("~"), directoryPath.hasPrefix(homeDirectory + "/") {
            return "~" + directoryPath.dropFirst(homeDirectory.count) + "/"
        }
        return directoryPath == "/" ? "/" : directoryPath + "/"
    }
}
