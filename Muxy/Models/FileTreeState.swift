import Foundation

@MainActor
@Observable
final class FileTreeState {
    enum FileStatus: Equatable {
        case modified
        case added
        case untracked
        case renamed
        case conflict
    }

    enum PendingEntryKind {
        case file
        case folder
    }

    struct PendingNewEntry: Equatable {
        let parentPath: String
        let kind: PendingEntryKind
        let token: UUID
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
    var selectedPaths: Set<String> = []
    var selectionAnchorPath: String?
    var pendingRenamePath: String?
    var pendingNewEntry: PendingNewEntry?
    var pendingDeletePaths: [String] = []
    var cutPaths: Set<String> = []
    var dropHighlightPath: String?

    @ObservationIgnored private var watcher: FileSystemWatcher?
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

    func refreshDirectory(path: String) {
        let normalized = path.hasSuffix("/") ? String(path.dropLast()) : path
        if normalized == normalizedRootPath {
            reloadRoot()
        } else {
            reloadChildren(of: normalized)
        }
        refreshStatuses()
    }

    func expand(path: String) {
        let normalized = path.hasSuffix("/") ? String(path.dropLast()) : path
        guard normalized != normalizedRootPath else { return }
        guard !expanded.contains(normalized) else { return }
        expanded.insert(normalized)
        reloadChildren(of: normalized)
    }

    func parentDirectory(of path: String) -> String {
        let normalized = path.hasSuffix("/") ? String(path.dropLast()) : path
        return (normalized as NSString).deletingLastPathComponent
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
        guard showOnlyChanges else { return rootEntries }
        return rootEntries.filter { entryHasChanges($0) }
    }

    func visibleChildren(of entry: FileTreeEntry) -> [FileTreeEntry]? {
        guard let entries = children[entry.absolutePath] else { return nil }
        guard showOnlyChanges else { return entries }
        return entries.filter { entryHasChanges($0) }
    }

    func entryHasChanges(_ entry: FileTreeEntry) -> Bool {
        if entry.isDirectory { return dirHasChange.contains(entry.absolutePath) }
        return statuses[entry.absolutePath] != nil
    }

    func selectOnly(_ path: String) {
        selectedFilePath = path
        selectedPaths = [path]
        selectionAnchorPath = path
    }

    func toggleSelection(_ path: String) {
        if selectedPaths.contains(path) {
            selectedPaths.remove(path)
            if selectedFilePath == path {
                selectedFilePath = selectedPaths.first
            }
        } else {
            selectedPaths.insert(path)
            selectedFilePath = path
        }
        selectionAnchorPath = path
    }

    func extendSelection(to path: String) {
        let ordered = visiblePathsInOrder()
        guard let endIdx = ordered.firstIndex(of: path) else {
            selectedPaths.insert(path)
            selectedFilePath = path
            selectionAnchorPath = path
            return
        }
        let anchor = selectionAnchorPath ?? selectedFilePath ?? path
        guard let startIdx = ordered.firstIndex(of: anchor) else {
            selectedPaths.insert(path)
            selectedFilePath = path
            selectionAnchorPath = path
            return
        }
        let range = startIdx <= endIdx ? startIdx ... endIdx : endIdx ... startIdx
        selectedPaths = Set(ordered[range])
        selectedFilePath = path
    }

    func clearSelection() {
        selectedFilePath = nil
        selectedPaths = []
        selectionAnchorPath = nil
    }

    func isPathSelected(_ path: String) -> Bool {
        selectedPaths.contains(path)
    }

    func visiblePathsInOrder() -> [String] {
        var result: [String] = []
        for entry in visibleRootEntries() {
            appendVisible(entry, into: &result)
        }
        return result
    }

    enum FlatRowItem: Identifiable {
        case entry(FileTreeEntry, depth: Int)
        case pendingNew(PendingNewEntry, depth: Int)

        var id: String {
            switch self {
            case let .entry(entry, _):
                "e:\(entry.absolutePath)"
            case let .pendingNew(pending, _):
                "p:\(pending.token.uuidString)"
            }
        }
    }

    func flatVisibleRows() -> [FlatRowItem] {
        var result: [FlatRowItem] = []
        for entry in visibleRootEntries() {
            appendFlat(entry, depth: 0, into: &result)
        }
        if let pending = pendingNewEntry, pending.parentPath == normalizedRootPath {
            result.append(.pendingNew(pending, depth: 0))
        }
        return result
    }

    private func appendFlat(_ entry: FileTreeEntry, depth: Int, into result: inout [FlatRowItem]) {
        result.append(.entry(entry, depth: depth))
        guard entry.isDirectory, expanded.contains(entry.absolutePath),
              let children = visibleChildren(of: entry)
        else { return }
        for child in children {
            appendFlat(child, depth: depth + 1, into: &result)
        }
        if let pending = pendingNewEntry, pending.parentPath == entry.absolutePath {
            result.append(.pendingNew(pending, depth: depth + 1))
        }
    }

    func entry(at path: String) -> FileTreeEntry? {
        let parent = parentDirectory(of: path)
        let candidates: [FileTreeEntry]
        if parent == normalizedRootPath {
            candidates = visibleRootEntries()
        } else if let parentEntry = entry(at: parent),
                  let kids = visibleChildren(of: parentEntry)
        {
            candidates = kids
        } else {
            return nil
        }
        return candidates.first { $0.absolutePath == path }
    }

    func moveSelection(by delta: Int) {
        let ordered = visiblePathsInOrder()
        guard !ordered.isEmpty else { return }
        let currentIndex = selectedFilePath.flatMap { ordered.firstIndex(of: $0) }
        let targetIndex: Int = if let currentIndex {
            max(0, min(ordered.count - 1, currentIndex + delta))
        } else {
            delta >= 0 ? 0 : ordered.count - 1
        }
        selectOnly(ordered[targetIndex])
    }

    func collapseOrJumpToParent() {
        guard let path = selectedFilePath else { return }
        if let entry = entry(at: path), entry.isDirectory, expanded.contains(path) {
            expanded.remove(path)
            return
        }
        let parent = parentDirectory(of: path)
        guard parent != normalizedRootPath else { return }
        guard visiblePathsInOrder().contains(parent) else { return }
        selectOnly(parent)
    }

    func expandOrDescend() {
        guard let path = selectedFilePath,
              let entry = entry(at: path),
              entry.isDirectory
        else { return }
        if !expanded.contains(path) {
            expand(path: path)
            return
        }
        let ordered = visiblePathsInOrder()
        guard let idx = ordered.firstIndex(of: path), idx + 1 < ordered.count else { return }
        let next = ordered[idx + 1]
        guard next.hasPrefix(path + "/") else { return }
        selectOnly(next)
    }

    func activateSelection(open: (String) -> Void) {
        guard let path = selectedFilePath, let entry = entry(at: path) else { return }
        if entry.isDirectory {
            toggle(entry)
            return
        }
        open(path)
    }

    private func appendVisible(_ entry: FileTreeEntry, into result: inout [String]) {
        result.append(entry.absolutePath)
        guard entry.isDirectory, expanded.contains(entry.absolutePath),
              let children = visibleChildren(of: entry)
        else { return }
        for child in children {
            appendVisible(child, into: &result)
        }
    }

    func revealFile(at filePath: String) {
        selectedFilePath = filePath
        selectedPaths = [filePath]
        selectionAnchorPath = filePath
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
        watcher = FileSystemWatcher(directoryPath: rootPath) { [weak self] in
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

        var environment = ProcessInfo.processInfo.environment
        environment["GIT_OPTIONAL_LOCKS"] = "0"
        process.environment = environment

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
            guard let status = mapStatus(file) else { continue }
            let absolute = normalizedRoot + "/" + file.path
            let trimmed = absolute.hasSuffix("/") ? String(absolute.dropLast()) : absolute
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

    nonisolated private static func mapStatus(_ file: GitStatusFile) -> FileStatus? {
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
            return nil
        }
        if x == "R" || y == "R" || x == "C" || y == "C" {
            return .renamed
        }
        return .modified
    }
}
