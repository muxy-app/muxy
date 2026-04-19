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
            if children[entry.absolutePath] == nil {
                reloadChildren(of: entry.absolutePath)
            }
        }
    }

    func isExpanded(_ entry: FileTreeEntry) -> Bool {
        expanded.contains(entry.absolutePath)
    }

    func childrenOf(_ entry: FileTreeEntry) -> [FileTreeEntry]? {
        children[entry.absolutePath]
    }

    func visibleRootEntries() -> [FileTreeEntry] {
        guard showOnlyChanges else { return rootEntries }
        return rootEntries.filter { entryHasChanges($0) }
    }

    func visibleChildren(of entry: FileTreeEntry) -> [FileTreeEntry]? {
        guard let all = children[entry.absolutePath] else { return nil }
        guard showOnlyChanges else { return all }
        return all.filter { entryHasChanges($0) }
    }

    func entryHasChanges(_ entry: FileTreeEntry) -> Bool {
        if entry.isDirectory { return dirHasChange.contains(entry.absolutePath) }
        return statuses[entry.absolutePath] != nil
    }

    func revealFile(at filePath: String) {
        selectedFilePath = filePath
        let normalizedRoot = rootPath.hasSuffix("/") ? String(rootPath.dropLast()) : rootPath
        guard filePath.hasPrefix(normalizedRoot + "/") else { return }
        let relative = String(filePath.dropFirst(normalizedRoot.count + 1))
        let components = relative.split(separator: "/").map(String.init)
        guard components.count > 1 else { return }
        var current = normalizedRoot
        for component in components.dropLast() {
            current += "/" + component
            if !expanded.contains(current) {
                expanded.insert(current)
                if children[current] == nil {
                    reloadChildren(of: current)
                }
            }
        }
    }

    func status(for absolutePath: String) -> FileStatus? {
        statuses[absolutePath]
    }

    func directoryHasChanges(_ absolutePath: String) -> Bool {
        dirHasChange.contains(absolutePath)
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
        process.arguments = ["-C", repoRoot, "status", "--porcelain=v1", "-z", "--untracked-files=normal"]

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

        guard let raw = String(data: outData, encoding: .utf8) else {
            return StatusResult(fileStatuses: [:], dirtyDirs: [])
        }

        let normalizedRoot = repoRoot.hasSuffix("/") ? String(repoRoot.dropLast()) : repoRoot
        var fileStatuses: [String: FileStatus] = [:]
        var dirtyDirs: Set<String> = []

        let entries = raw.split(separator: "\u{0}", omittingEmptySubsequences: true)
        var index = 0
        while index < entries.count {
            let entry = entries[index]
            index += 1
            guard entry.count >= 3 else { continue }
            let chars = Array(entry)
            let x = chars[0]
            let y = chars[1]
            let pathStart = entry.index(entry.startIndex, offsetBy: 3)
            let path = String(entry[pathStart...])

            if x == "R" || x == "C", index < entries.count {
                index += 1
            }

            let absolute = normalizedRoot + "/" + path
            let trimmed = absolute.hasSuffix("/") ? String(absolute.dropLast()) : absolute

            let status: FileStatus = if x == "U" || y == "U" || (x == "A" && y == "A") || (x == "D" && y == "D") {
                .conflict
            } else if x == "?" {
                .untracked
            } else if x == "A" || y == "A" {
                .added
            } else if x == "D" || y == "D" {
                .deleted
            } else if x == "R" || y == "R" {
                .renamed
            } else {
                .modified
            }

            fileStatuses[trimmed] = status

            var current = (trimmed as NSString).deletingLastPathComponent
            while current.count > normalizedRoot.count {
                if dirtyDirs.contains(current) { break }
                dirtyDirs.insert(current)
                current = (current as NSString).deletingLastPathComponent
            }
        }

        return StatusResult(fileStatuses: fileStatuses, dirtyDirs: dirtyDirs)
    }
}
