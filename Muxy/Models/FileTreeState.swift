import Foundation

@MainActor
@Observable
final class FileTreeState {
    enum FileStatus: Equatable {
        case modified
        case added
        case untracked
        case deleted
        case renamed
        case conflict
    }

    let rootPath: String
    private(set) var rootEntries: [FileTreeEntry] = []
    private(set) var children: [String: [FileTreeEntry]] = [:]
    private(set) var expanded: Set<String> = []
    private(set) var loadingPaths: Set<String> = []
    private(set) var hasLoadedRoot = false
    private(set) var statuses: [String: FileStatus] = [:]
    private(set) var dirHasChange: Set<String> = []
    var showOnlyChanges = false
    var selectedFilePath: String?

    @ObservationIgnored private var watcher: GitDirectoryWatcher?
    @ObservationIgnored nonisolated(unsafe) private var remoteChangeObserver: NSObjectProtocol?
    @ObservationIgnored private var refreshTask: Task<Void, Never>?
    @ObservationIgnored private var statusTask: Task<Void, Never>?

    init(rootPath: String) {
        self.rootPath = rootPath
        observeRepoChanges()
        installWatcher()
    }

    deinit {
        if let observer = remoteChangeObserver {
            NotificationCenter.default.removeObserver(observer)
        }
    }

    func loadRootIfNeeded() {
        guard !hasLoadedRoot else { return }
        hasLoadedRoot = true
        reloadRoot()
        refreshStatuses()
    }

    func refresh() {
        reloadRoot()
        for path in expanded {
            reloadChildren(of: path)
        }
        refreshStatuses()
    }

    func toggle(_ entry: FileTreeEntry) {
        guard entry.isDirectory else { return }
        if expanded.contains(entry.absolutePath) {
            expanded.remove(entry.absolutePath)
        } else {
            expanded.insert(entry.absolutePath)
            reloadChildren(of: entry.absolutePath)
        }
    }

    func isExpanded(_ entry: FileTreeEntry) -> Bool {
        expanded.contains(entry.absolutePath)
    }

    func childrenOf(_ entry: FileTreeEntry) -> [FileTreeEntry]? {
        children[entry.absolutePath]
    }

    func visibleRootEntries() -> [FileTreeEntry] {
        let entries = mergedEntries(in: normalizedRootPath, realEntries: rootEntries)
        guard showOnlyChanges else { return entries }
        return entries.filter { entryHasChanges($0) }
    }

    func visibleChildren(of entry: FileTreeEntry) -> [FileTreeEntry]? {
        let realEntries = children[entry.absolutePath] ?? []
        let entries = mergedEntries(in: entry.absolutePath, realEntries: realEntries)
        guard !entries.isEmpty || children[entry.absolutePath] != nil else { return nil }
        guard showOnlyChanges else { return entries }
        return entries.filter { entryHasChanges($0) }
    }

    func entryHasChanges(_ entry: FileTreeEntry) -> Bool {
        if entry.isDirectory { return dirHasChange.contains(entry.absolutePath) }
        return statuses[entry.absolutePath] != nil
    }

    func revealFile(at filePath: String) {
        selectedFilePath = filePath
        guard filePath.hasPrefix(normalizedRootPath + "/") else { return }
        let relative = String(filePath.dropFirst(normalizedRootPath.count + 1))
        let components = relative.split(separator: "/").map(String.init)
        guard components.count > 1 else { return }
        var current = normalizedRootPath
        for component in components.dropLast() {
            current += "/" + component
            if !expanded.contains(current) {
                expanded.insert(current)
                reloadChildren(of: current)
            }
        }
    }

    func status(for absolutePath: String) -> FileStatus? {
        statuses[absolutePath]
    }

    func directoryHasChanges(_ absolutePath: String) -> Bool {
        dirHasChange.contains(absolutePath)
    }

    private var normalizedRootPath: String {
        rootPath.hasSuffix("/") ? String(rootPath.dropLast()) : rootPath
    }

    private func reloadRoot() {
        let root = rootPath
        refreshTask?.cancel()
        refreshTask = Task { [weak self] in
            let entries = await FileTreeService.loadChildren(of: root, repoRoot: root)
            guard !Task.isCancelled, let self else { return }
            rootEntries = entries
        }
    }

    private func reloadChildren(of directoryPath: String) {
        let root = rootPath
        loadingPaths.insert(directoryPath)
        Task { [weak self] in
            let entries = await FileTreeService.loadChildren(of: directoryPath, repoRoot: root)
            guard !Task.isCancelled, let self else { return }
            children[directoryPath] = entries
            loadingPaths.remove(directoryPath)
        }
    }

    private func observeRepoChanges() {
        let path = rootPath
        remoteChangeObserver = NotificationCenter.default.addObserver(
            forName: .vcsRepoDidChange,
            object: nil,
            queue: .main
        ) { [weak self] notification in
            guard let notifiedPath = notification.userInfo?["repoPath"] as? String,
                  notifiedPath == path
            else { return }
            MainActor.assumeIsolated {
                self?.refresh()
            }
        }
    }

    private func installWatcher() {
        watcher = GitDirectoryWatcher(directoryPath: rootPath) { [weak self] in
            Task { @MainActor [weak self] in
                self?.refresh()
            }
        }
    }

    private func refreshStatuses() {
        let root = rootPath
        statusTask?.cancel()
        statusTask = Task { [weak self] in
            let result = await Self.loadStatuses(repoRoot: root)
            guard !Task.isCancelled, let self else { return }
            statuses = result.fileStatuses
            dirHasChange = result.dirtyDirs
        }
    }

    private struct StatusResult {
        let fileStatuses: [String: FileStatus]
        let dirtyDirs: Set<String>
    }

    nonisolated private static func loadStatuses(repoRoot: String) async -> StatusResult {
        await GitProcessRunner.offMain {
            loadStatusesSync(repoRoot: repoRoot)
        }
    }

    nonisolated private static func loadStatusesSync(repoRoot: String) -> StatusResult {
        guard let gitPath = GitProcessRunner.resolveExecutable("git") else {
            return StatusResult(fileStatuses: [:], dirtyDirs: [])
        }

        let process = Process()
        process.executableURL = URL(fileURLWithPath: gitPath)
        process.arguments = ["-C", repoRoot, "-c", "core.quotepath=false", "status", "--porcelain=v1", "-z", "--untracked-files=normal"]

        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe

        do {
            try process.run()
        } catch {
            return StatusResult(fileStatuses: [:], dirtyDirs: [])
        }

        let outData = (try? stdoutPipe.fileHandleForReading.readToEnd()) ?? Data()
        _ = try? stderrPipe.fileHandleForReading.readToEnd()
        process.waitUntilExit()

        let normalizedRoot = repoRoot.hasSuffix("/") ? String(repoRoot.dropLast()) : repoRoot
        var fileStatuses: [String: FileStatus] = [:]
        var dirtyDirs: Set<String> = []

        for file in GitStatusParser.parseStatusPorcelain(outData, stats: [:]) {
            let absolute = normalizedRoot + "/" + file.path
            let trimmed = absolute.hasSuffix("/") ? String(absolute.dropLast()) : absolute
            fileStatuses[trimmed] = mapStatus(file)

            var current = (trimmed as NSString).deletingLastPathComponent
            while current.count > normalizedRoot.count {
                if dirtyDirs.contains(current) { break }
                dirtyDirs.insert(current)
                current = (current as NSString).deletingLastPathComponent
            }
        }

        return StatusResult(fileStatuses: fileStatuses, dirtyDirs: dirtyDirs)
    }

    private func mergedEntries(in directoryPath: String, realEntries: [FileTreeEntry]) -> [FileTreeEntry] {
        let existingPaths = Set(realEntries.map(\.absolutePath))
        var entries = realEntries
        entries.append(contentsOf: syntheticEntries(in: directoryPath, excluding: existingPaths))
        entries.sort { lhs, rhs in
            if lhs.isDirectory != rhs.isDirectory { return lhs.isDirectory && !rhs.isDirectory }
            return lhs.name.localizedCaseInsensitiveCompare(rhs.name) == .orderedAscending
        }
        return entries
    }

    private func syntheticEntries(in directoryPath: String, excluding existingPaths: Set<String>) -> [FileTreeEntry] {
        let prefix = directoryPath.hasSuffix("/") ? directoryPath : directoryPath + "/"
        var entriesByPath: [String: FileTreeEntry] = [:]

        for absolutePath in statuses.keys where absolutePath.hasPrefix(prefix) {
            let remainder = String(absolutePath.dropFirst(prefix.count))
            guard !remainder.isEmpty else { continue }

            let components = remainder.split(separator: "/", maxSplits: 1, omittingEmptySubsequences: true)
            guard let first = components.first else { continue }

            let name = String(first)
            let childPath = prefix + name
            guard !existingPaths.contains(childPath), entriesByPath[childPath] == nil else { continue }

            let isDirectory = components.count > 1
            let relativePath: String = if childPath.hasPrefix(normalizedRootPath + "/") {
                String(childPath.dropFirst(normalizedRootPath.count + 1))
            } else {
                name
            }

            entriesByPath[childPath] = FileTreeEntry(
                name: name,
                absolutePath: childPath,
                relativePath: relativePath,
                isDirectory: isDirectory,
                isIgnored: false
            )
        }

        return Array(entriesByPath.values)
    }

    nonisolated private static func mapStatus(_ file: GitStatusFile) -> FileStatus {
        let x = file.xStatus
        let y = file.yStatus

        if x == "U" || y == "U" || (x == "A" && y == "A") || (x == "D" && y == "D") {
            return .conflict
        }
        if x == "?" && y == "?" {
            return .untracked
        }
        if x == "A" || y == "A" {
            return .added
        }
        if x == "D" || y == "D" {
            return .deleted
        }
        if x == "R" || y == "R" || x == "C" || y == "C" {
            return .renamed
        }
        return .modified
    }
}
