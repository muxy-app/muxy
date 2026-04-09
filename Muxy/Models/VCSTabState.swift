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
    var branchName: String?
    var pullRequestInfo: GitRepositoryService.PRInfo?

    var commitMessage = ""
    var branches: [String] = []
    var isCommitting = false
    var isPushing = false
    var isPulling = false
    var isSwitchingBranch = false
    var isLoadingBranches = false
    var statusMessage: String?
    var statusIsError = false

    var commits: [GitCommit] = []
    var isLoadingCommits = false
    var hasMoreCommits = true
    var stagedCollapsed = false
    var changesCollapsed = false
    var historyCollapsed = false
    var sectionRatios: [CGFloat] = [0.33, 0.33, 0.34]

    var stagedFiles: [GitStatusFile] {
        files.filter(\.isStaged)
    }

    var unstagedFiles: [GitStatusFile] {
        files.filter(\.isUnstaged)
    }

    var hasStagedChanges: Bool {
        !stagedFiles.isEmpty
    }

    @ObservationIgnored private let git = GitRepositoryService()
    @ObservationIgnored private var loadFilesTask: Task<Void, Never>?
    @ObservationIgnored private var branchTask: Task<Void, Never>?
    @ObservationIgnored private var prInfoTask: Task<Void, Never>?
    @ObservationIgnored private var loadBranchesTask: Task<Void, Never>?
    @ObservationIgnored private var loadDiffTasks: [String: Task<Void, Never>] = [:]
    @ObservationIgnored private var commitLogTask: Task<Void, Never>?
    @ObservationIgnored private var watcher: GitDirectoryWatcher?
    @ObservationIgnored private var isRefreshing = false
    @ObservationIgnored private var pendingRefresh = false
    private(set) var hasCompletedInitialLoad = false
    @ObservationIgnored private static let commitsPerPage = 100

    init(projectPath: String) {
        self.projectPath = projectPath
        startWatching()
    }

    deinit {
        loadFilesTask?.cancel()
        branchTask?.cancel()
        prInfoTask?.cancel()
        loadBranchesTask?.cancel()
        commitLogTask?.cancel()
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
        if !incremental, files.isEmpty {
            isLoadingFiles = true
        }
        isRefreshing = true
        pendingRefresh = false
        errorMessage = nil

        branchTask?.cancel()
        prInfoTask?.cancel()
        branchTask = Task { [weak self] in
            guard let self else { return }
            do {
                let branch = try await git.currentBranch(repoPath: projectPath)
                guard !Task.isCancelled else { return }
                branchName = branch
                fetchPRInfo(branch: branch)
            } catch {
                guard !Task.isCancelled else { return }
                branchName = nil
                pullRequestInfo = nil
            }
        }

        if !historyCollapsed, commits.isEmpty {
            loadCommits()
        }

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
                for file in newFiles where oldFilesByPath[file.path] != file {
                    changedPaths.insert(file.path)
                }

                let listChanged = files.map(\.path) != newFiles.map(\.path) || !changedPaths.isEmpty
                if listChanged {
                    files = newFiles
                }
                isLoadingFiles = false
                hasCompletedInitialLoad = true

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

    struct FileStats {
        let additions: Int?
        let deletions: Int?
        let binary: Bool
    }

    func displayedStats(for file: GitStatusFile) -> FileStats {
        if let loaded = diffsByPath[file.path] {
            return FileStats(additions: loaded.additions, deletions: loaded.deletions, binary: false)
        }
        return FileStats(additions: file.additions, deletions: file.deletions, binary: file.isBinary)
    }

    func loadBranches() {
        loadBranchesTask?.cancel()
        isLoadingBranches = true
        loadBranchesTask = Task { [weak self] in
            guard let self else { return }
            defer {
                self.isLoadingBranches = false
                self.loadBranchesTask = nil
            }
            do {
                let result = try await git.listBranches(repoPath: projectPath)
                guard !Task.isCancelled else { return }
                branches = result
            } catch {
                guard !Task.isCancelled else { return }
                branches = []
            }
        }
    }

    func switchBranch(_ name: String) {
        guard name != branchName else { return }
        isSwitchingBranch = true
        Task { [weak self] in
            guard let self else { return }
            defer { isSwitchingBranch = false }
            do {
                try await git.switchBranch(repoPath: projectPath, branch: name)
                guard !Task.isCancelled else { return }
                branchName = name
                commits = []
                showStatus("Switched to \(name)", isError: false)
                performRefresh(incremental: false)
            } catch {
                guard !Task.isCancelled else { return }
                showStatus(errorText(error), isError: true)
            }
        }
    }

    func stageFile(_ path: String) {
        performGitOperation {
            try await self.git.stageFiles(repoPath: self.projectPath, paths: [path])
        }
    }

    func unstageFile(_ path: String) {
        performGitOperation {
            try await self.git.unstageFiles(repoPath: self.projectPath, paths: [path])
        }
    }

    func stageAll() {
        performGitOperation {
            try await self.git.stageAll(repoPath: self.projectPath)
        }
    }

    func unstageAll() {
        performGitOperation {
            try await self.git.unstageAll(repoPath: self.projectPath)
        }
    }

    func discardFile(_ path: String) {
        let file = files.first { $0.path == path }
        let isUntracked = file?.xStatus == "?" && file?.yStatus == "?"
        performGitOperation {
            if isUntracked {
                try await self.git.discardFiles(repoPath: self.projectPath, paths: [], untrackedPaths: [path])
            } else {
                try await self.git.discardFiles(repoPath: self.projectPath, paths: [path], untrackedPaths: [])
            }
        }
    }

    func discardAll() {
        performGitOperation {
            try await self.git.discardAll(repoPath: self.projectPath)
        }
    }

    func commit() {
        let message = commitMessage.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !message.isEmpty else {
            showStatus("Enter a commit message.", isError: true)
            return
        }
        guard hasStagedChanges else {
            showStatus("No staged changes to commit.", isError: true)
            return
        }
        isCommitting = true
        Task { [weak self] in
            guard let self else { return }
            defer { isCommitting = false }
            do {
                let hash = try await git.commit(repoPath: projectPath, message: message)
                guard !Task.isCancelled else { return }
                commitMessage = ""
                commits = []
                showStatus("Committed \(hash)", isError: false)
                performRefresh(incremental: false)
            } catch {
                guard !Task.isCancelled else { return }
                showStatus(errorText(error), isError: true)
            }
        }
    }

    func push() {
        isPushing = true
        Task { [weak self] in
            guard let self else { return }
            defer { isPushing = false }
            do {
                try await git.push(repoPath: projectPath)
                guard !Task.isCancelled else { return }
                showStatus("Pushed", isError: false)
            } catch {
                guard !Task.isCancelled else { return }
                showStatus(errorText(error), isError: true)
            }
        }
    }

    func pull() {
        isPulling = true
        Task { [weak self] in
            guard let self else { return }
            defer { isPulling = false }
            do {
                try await git.pull(repoPath: projectPath)
                guard !Task.isCancelled else { return }
                showStatus("Pulled", isError: false)
                performRefresh(incremental: false)
            } catch {
                guard !Task.isCancelled else { return }
                showStatus(errorText(error), isError: true)
            }
        }
    }

    func loadCommits() {
        commitLogTask?.cancel()
        isLoadingCommits = true
        commitLogTask = Task { [weak self] in
            guard let self else { return }
            defer { isLoadingCommits = false }
            do {
                let result = try await git.commitLog(repoPath: projectPath, maxCount: Self.commitsPerPage, skip: 0)
                guard !Task.isCancelled else { return }
                commits = result
                hasMoreCommits = result.count == Self.commitsPerPage
            } catch {
                guard !Task.isCancelled else { return }
                commits = []
                hasMoreCommits = false
            }
        }
    }

    func loadMoreCommits() {
        guard !isLoadingCommits, hasMoreCommits else { return }
        isLoadingCommits = true
        let skip = commits.count
        commitLogTask?.cancel()
        commitLogTask = Task { [weak self] in
            guard let self else { return }
            defer { isLoadingCommits = false }
            do {
                let result = try await git.commitLog(repoPath: projectPath, maxCount: Self.commitsPerPage, skip: skip)
                guard !Task.isCancelled else { return }
                commits.append(contentsOf: result)
                hasMoreCommits = result.count == Self.commitsPerPage
            } catch {
                guard !Task.isCancelled else { return }
            }
        }
    }

    func cherryPick(_ hash: String) {
        Task { [weak self] in
            guard let self else { return }
            do {
                try await git.cherryPick(repoPath: projectPath, hash: hash)
                guard !Task.isCancelled else { return }
                commits = []
                showStatus("Cherry-picked \(String(hash.prefix(7)))", isError: false)
                performRefresh(incremental: false)
            } catch {
                guard !Task.isCancelled else { return }
                showStatus(errorText(error), isError: true)
            }
        }
    }

    func createBranch(name: String, from hash: String) {
        let trimmedName = name.trimmingCharacters(in: .whitespacesAndNewlines)
        Task { [weak self] in
            guard let self else { return }
            do {
                try await git.createBranch(repoPath: projectPath, name: trimmedName, startPoint: hash)
                guard !Task.isCancelled else { return }
                showStatus("Created branch \(trimmedName)", isError: false)
                loadBranches()
                loadCommits()
            } catch {
                guard !Task.isCancelled else { return }
                showStatus(errorText(error), isError: true)
            }
        }
    }

    func createTag(name: String, at hash: String) {
        let trimmedName = name.trimmingCharacters(in: .whitespacesAndNewlines)
        Task { [weak self] in
            guard let self else { return }
            do {
                try await git.createTag(repoPath: projectPath, name: trimmedName, hash: hash)
                guard !Task.isCancelled else { return }
                showStatus("Created tag \(trimmedName)", isError: false)
                loadCommits()
            } catch {
                guard !Task.isCancelled else { return }
                showStatus(errorText(error), isError: true)
            }
        }
    }

    func checkoutDetached(_ hash: String) {
        Task { [weak self] in
            guard let self else { return }
            do {
                try await git.checkoutDetached(repoPath: projectPath, hash: hash)
                guard !Task.isCancelled else { return }
                commits = []
                showStatus("Checked out \(String(hash.prefix(7)))", isError: false)
                performRefresh(incremental: false)
            } catch {
                guard !Task.isCancelled else { return }
                showStatus(errorText(error), isError: true)
            }
        }
    }

    private func performGitOperation(_ operation: @escaping () async throws -> Void) {
        Task { [weak self] in
            guard let self else { return }
            do {
                try await operation()
                guard !Task.isCancelled else { return }
                performRefresh(incremental: true)
            } catch {
                guard !Task.isCancelled else { return }
                showStatus(errorText(error), isError: true)
            }
        }
    }

    private func fetchPRInfo(branch: String) {
        prInfoTask?.cancel()
        prInfoTask = Task { [weak self] in
            guard let self else { return }
            let prInfo = await git.pullRequestInfo(repoPath: projectPath, branch: branch)
            guard !Task.isCancelled else { return }
            pullRequestInfo = prInfo
        }
    }

    private func showStatus(_ message: String, isError: Bool) {
        if isError {
            statusMessage = message
            statusIsError = true
        } else {
            ToastState.shared.show(message)
        }
    }

    private func errorText(_ error: Error) -> String {
        (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
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
