import Foundation

@MainActor
@Observable
final class VCSTabState {
    enum ViewMode: String, CaseIterable, Identifiable {
        case unified
        case split

        var id: String { rawValue }

        var title: String {
            switch self {
            case .unified:
                "Unified"
            case .split:
                "Split"
            }
        }
    }

    struct LoadedDiff {
        let rows: [DiffDisplayRow]
        let additions: Int
        let deletions: Int
        let truncated: Bool
    }

    let projectPath: String
    var files: [GitStatusFile] = []
    var mode: ViewMode = .unified
    var hideWhitespace = false
    var expandedFilePaths: Set<String> = []
    var isLoadingFiles = false
    var errorMessage: String?
    var diffsByPath: [String: LoadedDiff] = [:]
    var loadingDiffPaths: Set<String> = []
    var diffErrorsByPath: [String: String] = [:]

    @ObservationIgnored private let git = GitRepositoryService()
    @ObservationIgnored private var loadFilesTask: Task<Void, Never>?
    @ObservationIgnored private var loadDiffTasks: [String: Task<Void, Never>] = [:]
    @ObservationIgnored private var watcher: GitDirectoryWatcher?
    @ObservationIgnored private var isRefreshing = false
    @ObservationIgnored private var pendingRefresh = false

    init(projectPath: String) {
        self.projectPath = projectPath
        startWatching()
    }

    deinit {
        loadFilesTask?.cancel()
        loadDiffTasks.values.forEach { $0.cancel() }
    }

    private func startWatching() {
        watcher = GitDirectoryWatcher(directoryPath: projectPath) { [weak self] in
            Task { @MainActor [weak self] in
                self?.watcherDidFire()
            }
        }
    }

    private func watcherDidFire() {
        guard !isRefreshing else {
            pendingRefresh = true
            return
        }
        performRefresh(incremental: true)
    }

    func refresh() {
        performRefresh(incremental: false)
    }

    private func performRefresh(incremental: Bool) {
        loadFilesTask?.cancel()
        if !incremental {
            isLoadingFiles = true
        }
        isRefreshing = true
        pendingRefresh = false
        errorMessage = nil

        loadFilesTask = Task { [weak self] in
            guard let self else { return }
            defer {
                self.isRefreshing = false
                if self.pendingRefresh {
                    self.pendingRefresh = false
                    self.performRefresh(incremental: true)
                }
            }
            do {
                let newFiles = try await git.changedFiles(repoPath: projectPath, ignoreWhitespace: hideWhitespace)
                guard !Task.isCancelled else { return }

                let oldFilesByPath = Dictionary(files.map { ($0.path, $0) }, uniquingKeysWith: { _, b in b })
                let newFilesByPath = Dictionary(newFiles.map { ($0.path, $0) }, uniquingKeysWith: { _, b in b })

                let validPaths = Set(newFiles.map(\.path))
                let removedPaths = Set(oldFilesByPath.keys).subtracting(validPaths)

                if !removedPaths.isEmpty {
                    expandedFilePaths = expandedFilePaths.intersection(validPaths)
                    for path in removedPaths {
                        diffsByPath.removeValue(forKey: path)
                        loadingDiffPaths.remove(path)
                        diffErrorsByPath.removeValue(forKey: path)
                        loadDiffTasks[path]?.cancel()
                        loadDiffTasks.removeValue(forKey: path)
                    }
                }

                var changedPaths: Set<String> = []
                for file in newFiles {
                    if oldFilesByPath[file.path] != file {
                        changedPaths.insert(file.path)
                    }
                }

                let listChanged = files.map(\.path) != newFiles.map(\.path) || !changedPaths.isEmpty
                if listChanged {
                    files = newFiles
                }
                isLoadingFiles = false

                if incremental {
                    for path in expandedFilePaths where changedPaths.contains(path) {
                        loadDiff(filePath: path, forceFull: false)
                    }
                } else {
                    for path in expandedFilePaths {
                        loadDiff(filePath: path, forceFull: false)
                    }
                }
            } catch {
                guard !Task.isCancelled else { return }
                files = []
                expandedFilePaths = []
                diffsByPath = [:]
                loadingDiffPaths = []
                diffErrorsByPath = [:]
                loadDiffTasks.values.forEach { $0.cancel() }
                loadDiffTasks = [:]
                errorMessage = (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
                isLoadingFiles = false
            }
        }
    }

    func toggleExpanded(filePath: String) {
        if expandedFilePaths.contains(filePath) {
            expandedFilePaths.remove(filePath)
            return
        }

        expandedFilePaths.insert(filePath)
        if diffsByPath[filePath] == nil {
            loadDiff(filePath: filePath, forceFull: false)
        }
    }

    func collapseAll() {
        expandedFilePaths.removeAll()
    }

    func expandAll() {
        for file in files {
            expandedFilePaths.insert(file.path)
            if diffsByPath[file.path] == nil {
                loadDiff(filePath: file.path, forceFull: false)
            }
        }
    }

    func loadFullDiff(filePath: String) {
        loadDiff(filePath: filePath, forceFull: true)
    }

    func toggleWhitespace() {
        hideWhitespace.toggle()
        diffsByPath.removeAll()
        performRefresh(incremental: false)
    }

    func displayedStats(for file: GitStatusFile) -> (additions: Int?, deletions: Int?, binary: Bool) {
        if let loaded = diffsByPath[file.path] {
            return (loaded.additions, loaded.deletions, false)
        }
        return (file.additions, file.deletions, file.isBinary)
    }

    private func loadDiff(filePath: String, forceFull: Bool) {
        loadDiffTasks[filePath]?.cancel()
        loadingDiffPaths.insert(filePath)
        diffErrorsByPath[filePath] = nil

        let lineLimit = forceFull ? nil : 20000
        let ignoreWhitespace = hideWhitespace

        loadDiffTasks[filePath] = Task { [weak self] in
            guard let self else { return }
            do {
                let result = try await git.patchAndCompare(
                    repoPath: projectPath,
                    filePath: filePath,
                    lineLimit: lineLimit,
                    ignoreWhitespace: ignoreWhitespace
                )
                guard !Task.isCancelled else { return }

                diffsByPath[filePath] = LoadedDiff(
                    rows: result.rows,
                    additions: result.additions,
                    deletions: result.deletions,
                    truncated: result.truncated
                )
                loadingDiffPaths.remove(filePath)
                loadDiffTasks.removeValue(forKey: filePath)
            } catch {
                guard !Task.isCancelled else { return }
                diffErrorsByPath[filePath] = (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
                loadingDiffPaths.remove(filePath)
                loadDiffTasks.removeValue(forKey: filePath)
            }
        }
    }
}
