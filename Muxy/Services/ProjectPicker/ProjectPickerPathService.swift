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

protocol ProjectPickerFileSystem: Sendable {
    func directoryState(atPath path: String) -> ProjectPickerFileSystemDirectoryState
    func isReadableFile(atPath path: String) -> Bool
    func contentsOfDirectory(atPath path: String) async throws -> [ProjectPickerFileSystemDirectoryEntry]
}

struct FileManagerProjectPickerFileSystem: ProjectPickerFileSystem {
    private var fileManager: FileManager { .default }

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

    func contentsOfDirectory(atPath path: String) async throws -> [ProjectPickerFileSystemDirectoryEntry] {
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
    let isRemote: Bool
    private let fileSystem: any ProjectPickerFileSystem

    init(
        homeDirectory: String = NSHomeDirectory(),
        fileSystem: any ProjectPickerFileSystem = FileManagerProjectPickerFileSystem(),
        isRemote: Bool = false
    ) {
        self.homeDirectory = homeDirectory
        self.fileSystem = fileSystem
        self.isRemote = isRemote
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
            standardizedConfirmPath: standardize(confirmPath),
            parentDisplayPath: parentDisplayPath(for: directoryPath),
            completionDisplayPrefix: completionDisplayPrefix(for: trimmedInput, directoryPath: directoryPath)
        )
    }

    func standardize(_ path: String) -> String {
        isRemote ? Self.standardizedRemotePath(path) : Self.standardizedPath(path)
    }

    static func standardizedRemotePath(_ path: String) -> String {
        var components: [String] = []
        let isAbsolute = path.hasPrefix("/")
        for segment in path.split(separator: "/", omittingEmptySubsequences: true) {
            if segment == "." { continue }
            if segment == "..", let last = components.last, last != ".." {
                components.removeLast()
                continue
            }
            components.append(String(segment))
        }
        let joined = components.joined(separator: "/")
        if isAbsolute { return "/" + joined }
        return joined.isEmpty ? "." : joined
    }

    func typedPathState(path: String) -> ProjectPickerTypedPathState {
        switch fileSystem.directoryState(atPath: standardize(path)) {
        case .missing:
            .missing
        case .directory:
            .directory
        case .notDirectory:
            .notDirectory
        }
    }

    func defaultLocationStatus(path: String) -> ProjectPickerDefaultLocationStatus {
        let standardizedPath = standardize(path)
        switch fileSystem.directoryState(atPath: standardizedPath) {
        case .missing:
            return .missing
        case .notDirectory:
            return .notDirectory
        case .directory:
            return fileSystem.isReadableFile(atPath: standardizedPath) ? .ready : .unreadable
        }
    }

    func directorySnapshot(for pathState: ProjectPickerPathState) async -> ProjectPickerDirectorySnapshot {
        switch await directoryContents(atPath: pathState.directoryPath) {
        case let .success(items):
            ProjectPickerDirectorySnapshot(rows: pathState.directoryItems(from: items), readFailed: false)
        case .failure:
            ProjectPickerDirectorySnapshot(rows: pathState.directoryReadFailureItems, readFailed: true)
        }
    }

    func directoryContents(atPath path: String) async -> Result<[ProjectPickerDirectoryItem], Error> {
        do {
            let items = try await fileSystem.contentsOfDirectory(atPath: path).compactMap(\.projectPickerDirectoryItem)
            return .success(items)
        } catch {
            return .failure(error)
        }
    }

    func snapshot(for pathState: ProjectPickerPathState, items: [ProjectPickerDirectoryItem]) -> ProjectPickerDirectorySnapshot {
        ProjectPickerDirectorySnapshot(rows: pathState.directoryItems(from: items), readFailed: false)
    }

    func expandedPath(_ path: String) -> String {
        let trimmedPath = path.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !isRemote else { return trimmedPath }
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

    func directoryDisplayPath(_ path: String) -> String {
        guard !isRemote else {
            let standardizedPath = standardize(path)
            return standardizedPath.hasSuffix("/") ? standardizedPath : standardizedPath + "/"
        }
        return abbreviatedDirectoryDisplayPath(path)
    }

    static func standardizedPath(_ path: String) -> String {
        URL(fileURLWithPath: path).standardizedFileURL.path
    }

    private func confirmPath(for trimmedInput: String) -> String {
        guard !trimmedInput.isEmpty else { return isRemote ? homeDirectory : "/" }
        let expandedPath = expandedPath(trimmedInput)
        if isRemote { return expandedPath }
        guard expandedPath.hasPrefix("/") else { return "/" + expandedPath }
        return expandedPath
    }

    private func directoryPath(for trimmedInput: String, expandedInput: String) -> String {
        if trimmedInput.isEmpty { return "/" }
        if trimmedInput == "~" { return standardize(homeDirectory) }
        guard !expandedInput.hasSuffix("/") else {
            return standardize(expandedInput)
        }
        return standardize(parentPath(of: expandedInput))
    }

    private func leafFilter(for trimmedInput: String) -> String {
        if trimmedInput.isEmpty || trimmedInput == "~" || trimmedInput.hasSuffix("/") { return "" }
        return lastComponent(of: trimmedInput)
    }

    private func parentDisplayPath(for directoryPath: String) -> String {
        guard directoryPath != "/" else { return "/" }
        let parent = standardize(parentPath(of: directoryPath))
        guard parent != homeDirectory else { return "~/" }
        guard parent.hasPrefix(homeDirectory + "/") else { return parent == "/" ? "/" : parent + "/" }
        return "~" + parent.dropFirst(homeDirectory.count) + "/"
    }

    private func parentPath(of path: String) -> String {
        guard isRemote else {
            return URL(fileURLWithPath: path).deletingLastPathComponent().path
        }
        guard let slashIndex = path.lastIndex(of: "/") else { return path }
        let parent = String(path[..<slashIndex])
        return parent.isEmpty ? "/" : parent
    }

    private func lastComponent(of path: String) -> String {
        guard isRemote else {
            return URL(fileURLWithPath: path).lastPathComponent
        }
        guard let slashIndex = path.lastIndex(of: "/") else { return path }
        return String(path[path.index(after: slashIndex)...])
    }

    private func completionDisplayPrefix(for trimmedInput: String, directoryPath: String) -> String {
        if trimmedInput.hasPrefix("~"), directoryPath == homeDirectory { return "~/" }
        if trimmedInput.hasPrefix("~"), directoryPath.hasPrefix(homeDirectory + "/") {
            return "~" + directoryPath.dropFirst(homeDirectory.count) + "/"
        }
        return directoryPath == "/" ? "/" : directoryPath + "/"
    }
}
