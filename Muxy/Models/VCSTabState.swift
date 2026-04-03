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
    var expandedFilePaths: Set<String> = []
    var isLoadingFiles = false
    var errorMessage: String?
    var diffsByPath: [String: LoadedDiff] = [:]
    var loadingDiffPaths: Set<String> = []
    var diffErrorsByPath: [String: String] = [:]

    @ObservationIgnored private let git = GitRepositoryService()
    @ObservationIgnored private var loadFilesTask: Task<Void, Never>?
    @ObservationIgnored private var loadDiffTasks: [String: Task<Void, Never>] = [:]

    init(projectPath: String) {
        self.projectPath = projectPath
    }

    deinit {
        loadFilesTask?.cancel()
        loadDiffTasks.values.forEach { $0.cancel() }
    }

    func refresh() {
        loadFilesTask?.cancel()
        isLoadingFiles = true
        errorMessage = nil

        loadFilesTask = Task { [weak self] in
            guard let self else { return }
            do {
                let files = try await git.changedFiles(repoPath: projectPath)
                guard !Task.isCancelled else { return }

                self.files = files
                let validPaths = Set(files.map(\.path))
                self.expandedFilePaths = self.expandedFilePaths.intersection(validPaths)
                self.diffsByPath = self.diffsByPath.filter { validPaths.contains($0.key) }
                self.loadingDiffPaths = self.loadingDiffPaths.intersection(validPaths)
                self.diffErrorsByPath = self.diffErrorsByPath.filter { validPaths.contains($0.key) }
                self.loadDiffTasks = self.loadDiffTasks.filter { validPaths.contains($0.key) }

                if self.expandedFilePaths.isEmpty, let first = files.first {
                    self.toggleExpanded(filePath: first.path)
                } else {
                    for path in self.expandedFilePaths where self.diffsByPath[path] == nil {
                        self.loadDiff(filePath: path, forceFull: false)
                    }
                }

                self.isLoadingFiles = false
            } catch {
                guard !Task.isCancelled else { return }
                self.files = []
                self.expandedFilePaths = []
                self.diffsByPath = [:]
                self.loadingDiffPaths = []
                self.diffErrorsByPath = [:]
                self.loadDiffTasks.values.forEach { $0.cancel() }
                self.loadDiffTasks = [:]
                self.errorMessage = (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
                self.isLoadingFiles = false
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

        loadDiffTasks[filePath] = Task { [weak self] in
            guard let self else { return }
            do {
                let result = try await git.patchAndCompare(repoPath: projectPath, filePath: filePath, lineLimit: lineLimit)
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
