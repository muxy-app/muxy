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

    enum FileListMode: String, CaseIterable, Identifiable {
        case flat
        case folders

        var id: String { rawValue }
    }

    typealias LoadedDiff = DiffCache.LoadedDiff

    enum PRLaunchState: Equatable {
        case hidden
        case ghMissing
        case canCreate
        case hasPR(GitRepositoryService.PRInfo)
    }

    enum PRBranchStrategy: Equatable {
        case useCurrent
        case createNew(name: String)
    }

    enum PRIncludeMode: Equatable {
        case all
        case stagedOnly
        case none
    }

    struct PRCreateRequest {
        let baseBranch: String
        let title: String
        let body: String
        let branchStrategy: PRBranchStrategy
        let includeMode: PRIncludeMode
        let draft: Bool
    }

    let projectPath: String
    var files: [GitStatusFile] = []
    var mode: ViewMode = .unified
    var fileListMode: FileListMode = .flat {
        didSet {
            guard isLoaded, fileListMode != oldValue else { return }
            VCSPersistedSettings.storeFileListMode(fileListMode, repoPath: projectPath)
        }
    }

    var expandedFilePaths: Set<String> = []
    var expandedStagedFolderPaths: Set<String> = []
    var expandedUnstagedFolderPaths: Set<String> = []
    var isLoadingFiles = false
    var errorMessage: String?
    let diffCache = DiffCache()
    var branchName: String?
    var pullRequestInfo: GitRepositoryService.PRInfo?
    var defaultBranch: String?
    var remoteBranches: [String] = []
    var isLoadingRemoteBranches = false
    var isGhInstalled = true
    var aheadBehind = GitRepositoryService.AheadBehind(ahead: 0, behind: 0, hasUpstream: false)
    var isOpeningPullRequest = false
    var openPullRequestError: String?
    var isMergingPullRequest = false
    var isClosingPullRequest = false
    var isRefreshingPullRequest = false
    var hasFetchedPullRequestInfo = false
    private(set) var isGitRepo = false
    private(set) var remoteWebURL: URL?

    var commitMessage = ""
    var branches: [String] = []
    var isCommitting = false
    var isPushing = false
    var isPulling = false
    var isSwitchingBranch = false
    var isLoadingBranches = false
    var statusMessage: String?
    var statusIsError = false
    var showPushUpstreamConfirmation = false

    var commits: [GitCommit] = []
    var isLoadingCommits = false
    var hasMoreCommits = true
    var stagedCollapsed = false {
        didSet { persistCollapseIfChanged(oldValue, stagedCollapsed) }
    }

    var changesCollapsed = false {
        didSet { persistCollapseIfChanged(oldValue, changesCollapsed) }
    }

    var historyCollapsed = false {
        didSet { persistCollapseIfChanged(oldValue, historyCollapsed) }
    }

    var pullRequestsCollapsed = true {
        didSet { persistCollapseIfChanged(oldValue, pullRequestsCollapsed) }
    }

    var changesVisible = true
    var historyVisible = true
    var pullRequestsVisible = true
    var sectionRatios: [CGFloat] = [0.25, 0.25, 0.25, 0.25] {
        didSet {
            guard isLoaded, sectionRatios != oldValue else { return }
            VCSPersistedSettings.storeSectionRatios(sectionRatios, repoPath: projectPath)
        }
    }

    @ObservationIgnored private var isLoaded = false

    var pullRequests: [GitRepositoryService.PRListItem] = []
    var isLoadingPullRequests = false
    var pullRequestsLastError: String?
    var pullRequestsLastFetched: Date?
    var pullRequestSearchQuery = ""
    var pullRequestStateFilter: GitRepositoryService.PRListFilter = .open
    var pullRequestAutoSyncMinutes: Int = 0
    var checkingOutPRNumber: Int?

    var stagedFiles: [GitStatusFile] {
        files.filter(\.isStaged)
    }

    var unstagedFiles: [GitStatusFile] {
        files.filter(\.isUnstaged)
    }

    var hasStagedChanges: Bool {
        !stagedFiles.isEmpty
    }

    var hasAnyChanges: Bool {
        !files.isEmpty
    }

    var isOnDefaultBranch: Bool {
        guard let branchName, let defaultBranch else { return false }
        return branchName == defaultBranch
    }

    var prLaunchState: PRLaunchState {
        if !isGhInstalled { return .ghMissing }
        if branchName == nil || !hasFetchedPullRequestInfo { return .hidden }
        if let info = pullRequestInfo { return .hasPR(info) }
        guard canCreatePR else { return .hidden }
        return .canCreate
    }

    var canCreatePR: Bool {
        guard branchName != nil, pullRequestInfo == nil else { return false }
        if hasAnyChanges { return true }
        if isOnDefaultBranch { return false }
        return true
    }

    @ObservationIgnored private let git = GitRepositoryService()
    @ObservationIgnored private var loadFilesTask: Task<Void, Never>?
    @ObservationIgnored private var branchTask: Task<Void, Never>?
    @ObservationIgnored private var prInfoTask: Task<Void, Never>?
    @ObservationIgnored private var loadBranchesTask: Task<Void, Never>?
    @ObservationIgnored private var commitLogTask: Task<Void, Never>?
    @ObservationIgnored private var prListTask: Task<Void, Never>?
    @ObservationIgnored private var prAutoSyncTask: Task<Void, Never>?
    @ObservationIgnored private var watcher: FileSystemWatcher?
    @ObservationIgnored nonisolated(unsafe) private var remoteChangeObserver: NSObjectProtocol?
    @ObservationIgnored private var isRefreshing = false
    @ObservationIgnored private var pendingRefresh = false
    @ObservationIgnored private var lastFetchedHeadSha: String?
    private(set) var hasCompletedInitialLoad = false
    @ObservationIgnored private static let commitsPerPage = 100

    init(projectPath: String) {
        self.projectPath = projectPath
        pullRequestAutoSyncMinutes = VCSPersistedSettings.loadAutoSyncMinutes(repoPath: projectPath)
        let visibility = VCSPersistedSettings.loadSectionVisibility(repoPath: projectPath)
        changesVisible = visibility.changes
        historyVisible = visibility.history
        pullRequestsVisible = visibility.pullRequests
        let collapse = VCSPersistedSettings.loadSectionCollapse(repoPath: projectPath)
        stagedCollapsed = collapse.staged
        changesCollapsed = collapse.changes
        historyCollapsed = collapse.history
        pullRequestsCollapsed = collapse.pullRequests
        fileListMode = VCSPersistedSettings.loadFileListMode(repoPath: projectPath)
        let storedRatios = VCSPersistedSettings.loadSectionRatios(repoPath: projectPath)
        if storedRatios.count == sectionRatios.count {
            sectionRatios = storedRatios
        }
        isLoaded = true
        startWatching()
        observeRemoteChanges()
        rescheduleAutoSync()
    }

    deinit {
        loadFilesTask?.cancel()
        branchTask?.cancel()
        prInfoTask?.cancel()
        loadBranchesTask?.cancel()
        commitLogTask?.cancel()
        prListTask?.cancel()
        prAutoSyncTask?.cancel()
        diffCache.cancelAll()
        if let remoteChangeObserver {
            NotificationCenter.default.removeObserver(remoteChangeObserver)
        }
    }

    private func startWatching() {
        let gitPath = (projectPath as NSString).appendingPathComponent(".git")
        guard FileManager.default.fileExists(atPath: gitPath) else { return }
        watcher = FileSystemWatcher(directoryPath: projectPath) { [weak self] in
            Task { @MainActor [weak self] in
                self?.watcherDidFire()
            }
        }
    }

    private func observeRemoteChanges() {
        let path = projectPath
        remoteChangeObserver = NotificationCenter.default.addObserver(
            forName: .vcsRepoDidChange,
            object: nil,
            queue: .main
        ) { [weak self] notification in
            guard let notifiedPath = notification.userInfo?["repoPath"] as? String,
                  notifiedPath == path
            else { return }
            MainActor.assumeIsolated {
                self?.performRefresh(incremental: true, forcePRFetch: true)
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

    private func performRefresh(incremental: Bool, forcePRFetch: Bool = false) {
        loadFilesTask?.cancel()
        if !incremental, files.isEmpty {
            isLoadingFiles = true
        }
        isRefreshing = true
        pendingRefresh = false
        errorMessage = nil

        let refreshSignpost = GitSignpost.begin("performRefresh", incremental ? "incremental" : "full")

        branchTask?.cancel()
        prInfoTask?.cancel()
        let shouldForcePR = forcePRFetch || !incremental
        if shouldForcePR {
            isRefreshingPullRequest = true
        }
        branchTask = Task { [weak self] in
            guard let self else { return }
            do {
                async let branchValue = git.currentBranch(repoPath: projectPath)
                async let headValue = git.headSha(repoPath: projectPath)
                async let remoteURLValue = git.remoteWebURL(repoPath: projectPath)
                let branch = try await branchValue
                let head = await headValue
                let remoteURL = await remoteURLValue
                guard !Task.isCancelled else { return }

                isGitRepo = true
                remoteWebURL = remoteURL
                let branchChanged = branchName != branch
                if branchChanged {
                    hasFetchedPullRequestInfo = false
                    pullRequestInfo = nil
                    lastFetchedHeadSha = nil
                }
                branchName = branch

                let headChanged = head != lastFetchedHeadSha
                let neverFetched = !hasFetchedPullRequestInfo
                if shouldForcePR || branchChanged || headChanged || neverFetched {
                    fetchPRInfo(branch: branch, headSha: head, forceFresh: shouldForcePR)
                }

                let counts = await git.aheadBehind(repoPath: projectPath, branch: branch)
                guard !Task.isCancelled else { return }
                aheadBehind = counts
            } catch {
                guard !Task.isCancelled else { return }
                branchName = nil
                pullRequestInfo = nil
                hasFetchedPullRequestInfo = false
                lastFetchedHeadSha = nil
                aheadBehind = .init(ahead: 0, behind: 0, hasUpstream: false)
                isRefreshingPullRequest = false
            }
        }

        if !historyCollapsed, !incremental || commits.isEmpty {
            loadCommits()
        }

        loadFilesTask = Task { [weak self] in
            guard let self else { return }
            defer {
                self.isRefreshing = false
                GitSignpost.end("performRefresh", refreshSignpost)
                if self.pendingRefresh {
                    self.pendingRefresh = false
                    self.performRefresh(incremental: true)
                }
            }
            do {
                let newFiles = try await git.changedFiles(repoPath: projectPath)
                guard !Task.isCancelled else { return }

                let oldFilesByPath = Dictionary(files.map { ($0.path, $0) }, uniquingKeysWith: { _, b in b })

                let validPaths = Set(newFiles.map(\.path))
                let removedPaths = Set(oldFilesByPath.keys).subtracting(validPaths)

                if !removedPaths.isEmpty {
                    expandedFilePaths = expandedFilePaths.intersection(validPaths)
                    expandedStagedFolderPaths = expandedStagedFolderPaths.filter { folderPath in
                        validPaths.contains(where: { $0.hasPrefix(folderPath + "/") })
                    }
                    expandedUnstagedFolderPaths = expandedUnstagedFolderPaths.filter { folderPath in
                        validPaths.contains(where: { $0.hasPrefix(folderPath + "/") })
                    }
                    for path in removedPaths {
                        diffCache.evict(path)
                    }
                }

                var changedPaths: Set<String> = []
                for file in newFiles {
                    guard let old = oldFilesByPath[file.path] else {
                        changedPaths.insert(file.path)
                        continue
                    }
                    if Self.fileChanged(old: old, new: file) {
                        changedPaths.insert(file.path)
                        diffCache.evict(file.path)
                    }
                }

                let listChanged = files.map(\.path) != newFiles.map(\.path) || !changedPaths.isEmpty
                if listChanged {
                    files = newFiles
                }
                isLoadingFiles = false
                hasCompletedInitialLoad = true

                for path in expandedFilePaths where validPaths.contains(path) {
                    loadDiff(filePath: path, forceFull: false)
                }

                if !incremental {
                    NotificationCenter.default.post(
                        name: .vcsDidRefresh,
                        object: nil,
                        userInfo: ["repoPath": projectPath]
                    )
                }
            } catch {
                guard !Task.isCancelled else { return }
                files = []
                expandedFilePaths = []
                expandedStagedFolderPaths = []
                expandedUnstagedFolderPaths = []
                diffCache.clearAll()
                errorMessage = (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
                isLoadingFiles = false
            }
        }
    }

    private static func fileChanged(old: GitStatusFile, new: GitStatusFile) -> Bool {
        old.xStatus != new.xStatus
            || old.yStatus != new.yStatus
            || old.isBinary != new.isBinary
            || old.oldPath != new.oldPath
            || old.additions != new.additions
            || old.deletions != new.deletions
    }

    func toggleExpanded(filePath: String) {
        if expandedFilePaths.contains(filePath) {
            expandedFilePaths.remove(filePath)
            diffCache.cancelLoad(for: filePath)
            return
        }

        expandedFilePaths.insert(filePath)
        if diffCache.hasDiff(for: filePath) {
            diffCache.touch(filePath)
        } else {
            loadDiff(filePath: filePath, forceFull: false)
        }
    }

    func collapseAll() {
        expandedFilePaths.removeAll()
        diffCache.collapseAll()
    }

    func expandAll() {
        setExpanded(files: files, expanded: true)
    }

    func setExpanded(files: [GitStatusFile], expanded: Bool) {
        if expanded {
            var updated = expandedFilePaths
            var toLoad: [String] = []
            for file in files where !updated.contains(file.path) {
                updated.insert(file.path)
                if diffCache.hasDiff(for: file.path) {
                    diffCache.touch(file.path)
                } else {
                    toLoad.append(file.path)
                }
            }
            expandedFilePaths = updated
            for path in toLoad {
                loadDiff(filePath: path, forceFull: false)
            }
            return
        }

        var updated = expandedFilePaths
        for file in files where updated.contains(file.path) {
            updated.remove(file.path)
            diffCache.cancelLoad(for: file.path)
        }
        expandedFilePaths = updated
    }

    func toggleFolderExpanded(_ folderPath: String, isStaged: Bool) {
        if isStaged {
            if expandedStagedFolderPaths.contains(folderPath) {
                expandedStagedFolderPaths.remove(folderPath)
                return
            }
            expandedStagedFolderPaths.insert(folderPath)
            return
        }

        if expandedUnstagedFolderPaths.contains(folderPath) {
            expandedUnstagedFolderPaths.remove(folderPath)
            return
        }
        expandedUnstagedFolderPaths.insert(folderPath)
    }

    func isFolderExpanded(_ folderPath: String, isStaged: Bool) -> Bool {
        isStaged ? expandedStagedFolderPaths.contains(folderPath) : expandedUnstagedFolderPaths.contains(folderPath)
    }

    var stagedTreeRows: [VCSFileTree.Row] {
        VCSFileTree.rows(files: stagedFiles, expandedFolders: expandedStagedFolderPaths)
    }

    var unstagedTreeRows: [VCSFileTree.Row] {
        VCSFileTree.rows(files: unstagedFiles, expandedFolders: expandedUnstagedFolderPaths)
    }

    func loadFullDiff(filePath: String) {
        loadDiff(filePath: filePath, forceFull: true)
    }

    struct FileStats {
        let additions: Int?
        let deletions: Int?
        let binary: Bool
    }

    func displayedStats(for file: GitStatusFile) -> FileStats {
        if let loaded = diffCache.diff(for: file.path) {
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
                BranchCache.shared.update(projectPath: projectPath, branches: result)
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

    func createAndSwitchBranch(_ name: String) {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        isSwitchingBranch = true
        Task { [weak self] in
            guard let self else { return }
            defer { isSwitchingBranch = false }
            do {
                try await git.createAndSwitchBranch(repoPath: projectPath, name: trimmed)
                guard !Task.isCancelled else { return }
                branchName = trimmed
                commits = []
                showStatus("Created and switched to \(trimmed)", isError: false)
                loadBranches()
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
                performRefresh(incremental: false, forcePRFetch: true)
            } catch GitRepositoryService.GitError.noUpstreamBranch {
                guard !Task.isCancelled else { return }
                showPushUpstreamConfirmation = true
            } catch {
                guard !Task.isCancelled else { return }
                showStatus(errorText(error), isError: true)
            }
        }
    }

    func pushSetUpstream() {
        guard let branch = branchName else { return }
        isPushing = true
        Task { [weak self] in
            guard let self else { return }
            defer { isPushing = false }
            do {
                try await git.pushSetUpstream(repoPath: projectPath, branch: branch)
                guard !Task.isCancelled else { return }
                showStatus("Pushed to origin/\(branch)", isError: false)
                performRefresh(incremental: false, forcePRFetch: true)
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

    func revert(_ hash: String, subject: String) {
        Task { [weak self] in
            guard let self else { return }
            do {
                try await git.revert(repoPath: projectPath, hash: hash)
                guard !Task.isCancelled else { return }
                commitMessage = "Revert: \(subject)"
                showStatus("Reverted \(String(hash.prefix(7)))", isError: false)
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

    private func fetchPRInfo(branch: String, headSha: String?, forceFresh: Bool) {
        prInfoTask?.cancel()
        prInfoTask = Task { [weak self] in
            guard let self else { return }
            defer { if forceFresh { isRefreshingPullRequest = false } }
            async let ghInstalledValue = git.isGhInstalled()
            async let defaultBranchValue = git.defaultBranch(repoPath: projectPath)
            let ghInstalled = await ghInstalledValue
            let defaultBranchResult = await defaultBranchValue
            guard !Task.isCancelled else { return }
            isGhInstalled = ghInstalled
            defaultBranch = defaultBranchResult

            guard ghInstalled else {
                pullRequestInfo = nil
                hasFetchedPullRequestInfo = true
                lastFetchedHeadSha = headSha
                return
            }

            guard let headSha else {
                pullRequestInfo = nil
                hasFetchedPullRequestInfo = true
                return
            }

            let prInfo = await git.cachedPullRequestInfo(
                repoPath: projectPath,
                branch: branch,
                headSha: headSha,
                forceFresh: forceFresh
            )
            guard !Task.isCancelled else { return }
            pullRequestInfo = prInfo
            hasFetchedPullRequestInfo = true
            lastFetchedHeadSha = headSha
        }
    }

    func loadRemoteBranches() {
        guard !isLoadingRemoteBranches else { return }
        isLoadingRemoteBranches = true
        Task { [weak self] in
            guard let self else { return }
            defer { isLoadingRemoteBranches = false }
            do {
                let result = try await git.listRemoteBranches(repoPath: projectPath)
                guard !Task.isCancelled else { return }
                remoteBranches = result
            } catch {
                guard !Task.isCancelled else { return }
                remoteBranches = []
            }
        }
    }

    func refreshPullRequest() {
        guard let branch = branchName else { return }
        guard !isRefreshingPullRequest else { return }
        isRefreshingPullRequest = true
        Task { [weak self] in
            guard let self else { return }
            let head = await git.headSha(repoPath: projectPath)
            guard !Task.isCancelled else {
                isRefreshingPullRequest = false
                return
            }
            fetchPRInfo(branch: branch, headSha: head, forceFresh: true)
        }
    }

    func openPullRequest(_ request: PRCreateRequest) {
        let trimmedTitle = request.title.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedBase = request.baseBranch.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedTitle.isEmpty, !trimmedBase.isEmpty else {
            openPullRequestError = "Title and target branch are required."
            return
        }
        guard branchName != nil else {
            openPullRequestError = "No current branch."
            return
        }
        guard !isOpeningPullRequest else { return }

        isOpeningPullRequest = true
        openPullRequestError = nil

        let normalized = PRCreateRequest(
            baseBranch: trimmedBase,
            title: trimmedTitle,
            body: request.body,
            branchStrategy: request.branchStrategy,
            includeMode: request.includeMode,
            draft: request.draft
        )

        Task { [weak self] in
            guard let self else { return }
            defer { isOpeningPullRequest = false }
            do {
                try await performPRFlow(normalized)
            } catch {
                guard !Task.isCancelled else { return }
                openPullRequestError = errorText(error)
            }
        }
    }

    private func performPRFlow(_ request: PRCreateRequest) async throws {
        let targetBranch = try await resolvePRTargetBranch(
            strategy: request.branchStrategy,
            baseBranch: request.baseBranch
        )

        if Task.isCancelled { return }

        try await stageAndCommitForPR(
            includeMode: request.includeMode,
            title: request.title,
            body: request.body
        )

        if Task.isCancelled { return }

        try await git.pushSetUpstream(repoPath: projectPath, branch: targetBranch)

        if Task.isCancelled { return }

        let info = try await git.createPullRequest(
            repoPath: projectPath,
            branch: targetBranch,
            baseBranch: request.baseBranch,
            title: request.title,
            body: request.body,
            draft: request.draft
        )

        if Task.isCancelled { return }

        pullRequestInfo = info
        commits = []
        ToastState.shared.show("Pull request #\(info.number) opened")
        loadBranches()
        performRefresh(incremental: false)
    }

    private func resolvePRTargetBranch(
        strategy: PRBranchStrategy,
        baseBranch _: String
    ) async throws -> String {
        switch strategy {
        case .useCurrent:
            guard let current = branchName else {
                throw GitRepositoryService.GitError.commandFailed("No current branch.")
            }
            return current
        case let .createNew(name):
            let trimmedName = name.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmedName.isEmpty else {
                throw GitRepositoryService.GitError.commandFailed("Branch name is required.")
            }
            try await git.createAndSwitchBranch(repoPath: projectPath, name: trimmedName)
            branchName = trimmedName
            return trimmedName
        }
    }

    private func stageAndCommitForPR(includeMode: PRIncludeMode, title: String, body: String) async throws {
        switch includeMode {
        case .all:
            try await git.stageAll(repoPath: projectPath)
        case .stagedOnly,
             .none:
            break
        }

        if includeMode == .none { return }

        let status = try await git.changedFiles(repoPath: projectPath)
        if status.contains(where: \.isStaged) {
            let trimmedBody = body.trimmingCharacters(in: .whitespacesAndNewlines)
            let message = trimmedBody.isEmpty ? title : "\(title)\n\n\(trimmedBody)"
            _ = try await git.commit(repoPath: projectPath, message: message)
        }
    }

    func mergePullRequest(
        method: GitRepositoryService.PRMergeMethod = .squash,
        deleteBranch: Bool = true,
        onSuccess: @escaping (GitRepositoryService.PRInfo, String) -> Void
    ) {
        guard let info = pullRequestInfo, !isMergingPullRequest else { return }
        guard let branch = branchName else { return }
        isMergingPullRequest = true
        Task { [weak self] in
            guard let self else { return }
            defer { isMergingPullRequest = false }
            do {
                let skipDeleteForFork = deleteBranch && info.isCrossRepository
                try await git.mergePullRequest(
                    repoPath: projectPath,
                    number: info.number,
                    method: method,
                    deleteBranch: deleteBranch && !info.isCrossRepository
                )
                guard !Task.isCancelled else { return }
                pullRequestInfo = nil
                if skipDeleteForFork {
                    ToastState.shared.show("Branch lives on a fork — left intact.")
                }
                onSuccess(info, branch)
            } catch {
                guard !Task.isCancelled else { return }
                showStatus(errorText(error), isError: true)
            }
        }
    }

    func closePullRequest(onSuccess: @escaping () -> Void) {
        guard let info = pullRequestInfo, !isClosingPullRequest else { return }
        isClosingPullRequest = true
        Task { [weak self] in
            guard let self else { return }
            defer { isClosingPullRequest = false }
            do {
                try await git.closePullRequest(repoPath: projectPath, number: info.number)
                guard !Task.isCancelled else { return }
                pullRequestInfo = nil
                ToastState.shared.show("Closed PR #\(info.number)")
                onSuccess()
            } catch {
                guard !Task.isCancelled else { return }
                showStatus(errorText(error), isError: true)
            }
        }
    }

    func switchBranchAndRefresh(_ name: String) async {
        do {
            try await git.switchBranch(repoPath: projectPath, branch: name)
            branchName = name
            commits = []
            performRefresh(incremental: false)
        } catch {
            showStatus(errorText(error), isError: true)
        }
    }

    func deleteLocalBranch(_ name: String) async {
        do {
            try await GitWorktreeService.shared.deleteBranch(repoPath: projectPath, branch: name)
            loadBranches()
            showStatus("Deleted branch \(name)", isError: false)
        } catch {
            showStatus(errorText(error), isError: true)
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

    private func diffHints(for filePath: String) -> GitRepositoryService.DiffHints {
        guard let file = files.first(where: { $0.path == filePath }) else {
            return .unknown
        }
        let untrackedOrNew = file.xStatus == "?" || (file.xStatus == "A" && file.yStatus == " ")
        if untrackedOrNew {
            return GitRepositoryService.DiffHints(hasStaged: false, hasUnstaged: false, isUntrackedOrNew: true)
        }
        return GitRepositoryService.DiffHints(
            hasStaged: file.isStaged,
            hasUnstaged: file.isUnstaged,
            isUntrackedOrNew: false
        )
    }

    private func loadDiff(filePath: String, forceFull: Bool) {
        DiffLoader.load(
            DiffLoader.Request(
                repoPath: projectPath,
                filePath: filePath,
                hints: diffHints(for: filePath),
                forceFull: forceFull,
                pinnedPaths: expandedFilePaths
            ),
            cache: diffCache,
            git: git
        )
    }

    func ensureDiffLoaded(filePath: String, forceFull: Bool = false) {
        if !forceFull, diffCache.hasDiff(for: filePath) {
            diffCache.touch(filePath)
            return
        }
        loadDiff(filePath: filePath, forceFull: forceFull)
    }

    var filteredPullRequests: [GitRepositoryService.PRListItem] {
        let query = pullRequestSearchQuery.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !query.isEmpty else { return pullRequests }
        return pullRequests.filter { item in
            item.title.lowercased().contains(query)
                || item.author.lowercased().contains(query)
                || item.headBranch.lowercased().contains(query)
                || String(item.number).contains(query)
        }
    }

    func loadPullRequests() {
        prListTask?.cancel()
        isLoadingPullRequests = true
        pullRequestsLastError = nil
        let filter = pullRequestStateFilter
        prListTask = Task { [weak self] in
            guard let self else { return }
            defer { isLoadingPullRequests = false }
            do {
                let items = try await git.listPullRequests(repoPath: projectPath, filter: filter)
                guard !Task.isCancelled else { return }
                pullRequests = items
                pullRequestsLastFetched = Date()
            } catch {
                guard !Task.isCancelled else { return }
                pullRequests = []
                pullRequestsLastError = errorText(error)
            }
        }
    }

    func setPullRequestStateFilter(_ filter: GitRepositoryService.PRListFilter) {
        guard pullRequestStateFilter != filter else { return }
        pullRequestStateFilter = filter
        loadPullRequests()
    }

    func setPullRequestAutoSyncMinutes(_ minutes: Int) {
        pullRequestAutoSyncMinutes = minutes
        VCSPersistedSettings.storeAutoSyncMinutes(minutes, repoPath: projectPath)
        rescheduleAutoSync()
    }

    func checkoutPullRequest(_ item: GitRepositoryService.PRListItem) {
        guard checkingOutPRNumber == nil else { return }
        checkingOutPRNumber = item.number
        Task { [weak self] in
            guard let self else { return }
            defer { checkingOutPRNumber = nil }
            do {
                try await git.checkoutPullRequest(repoPath: projectPath, number: item.number)
                guard !Task.isCancelled else { return }
                ToastState.shared.show("Checked out PR #\(item.number)")
                commits = []
                performRefresh(incremental: false, forcePRFetch: true)
                loadBranches()
            } catch {
                guard !Task.isCancelled else { return }
                showStatus(errorText(error), isError: true)
            }
        }
    }

    private func rescheduleAutoSync() {
        prAutoSyncTask?.cancel()
        let minutes = pullRequestAutoSyncMinutes
        guard minutes > 0 else { return }
        let interval = UInt64(minutes) * 60 * 1_000_000_000
        prAutoSyncTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: interval)
                guard !Task.isCancelled else { return }
                await MainActor.run {
                    self?.loadPullRequests()
                }
            }
        }
    }

    func setChangesVisible(_ visible: Bool) {
        guard changesVisible != visible else { return }
        changesVisible = visible
        VCSPersistedSettings.storeSectionVisibility(currentVisibility, repoPath: projectPath)
    }

    func setHistoryVisible(_ visible: Bool) {
        guard historyVisible != visible else { return }
        historyVisible = visible
        VCSPersistedSettings.storeSectionVisibility(currentVisibility, repoPath: projectPath)
        if visible, commits.isEmpty {
            loadCommits()
        }
    }

    func setPullRequestsVisible(_ visible: Bool) {
        guard pullRequestsVisible != visible else { return }
        pullRequestsVisible = visible
        VCSPersistedSettings.storeSectionVisibility(currentVisibility, repoPath: projectPath)
    }

    private func persistCollapseIfChanged(_ oldValue: Bool, _ newValue: Bool) {
        guard isLoaded, oldValue != newValue else { return }
        VCSPersistedSettings.storeSectionCollapse(currentCollapse, repoPath: projectPath)
    }

    private var currentCollapse: VCSPersistedSettings.SectionCollapse {
        VCSPersistedSettings.SectionCollapse(
            staged: stagedCollapsed,
            changes: changesCollapsed,
            history: historyCollapsed,
            pullRequests: pullRequestsCollapsed
        )
    }

    private var currentVisibility: VCSPersistedSettings.SectionVisibility {
        VCSPersistedSettings.SectionVisibility(
            changes: changesVisible,
            history: historyVisible,
            pullRequests: pullRequestsVisible
        )
    }

    func loadDiffWithHints(
        filePath: String,
        hints: GitRepositoryService.DiffHints,
        forceFull: Bool = false
    ) {
        DiffLoader.load(
            DiffLoader.Request(
                repoPath: projectPath,
                filePath: filePath,
                hints: hints,
                forceFull: forceFull,
                pinnedPaths: expandedFilePaths.union([filePath])
            ),
            cache: diffCache,
            git: git
        )
    }
}

enum VCSFileTree {
    struct Folder: Equatable {
        let path: String
        let name: String
        let depth: Int
        let fileCount: Int
    }

    enum Row: Equatable, Identifiable {
        case folder(Folder)
        case file(GitStatusFile, depth: Int)

        var id: String {
            switch self {
            case let .folder(folder):
                "folder:\(folder.path)"
            case let .file(file, _):
                "file:\(file.path)"
            }
        }
    }

    static func rows(files: [GitStatusFile], expandedFolders: Set<String>) -> [Row] {
        let root = buildTree(files: files)
        return flattenRows(node: root, depth: 0, expandedFolders: expandedFolders)
    }

    private static func buildTree(files: [GitStatusFile]) -> VCSFileTreeNode {
        let root = VCSFileTreeNode()

        for file in files {
            let components = file.path.split(separator: "/").map(String.init)
            guard !components.isEmpty else { continue }

            if components.count == 1 {
                root.files.append(file)
                continue
            }

            var node = root
            var currentPath = ""

            for component in components.dropLast() {
                currentPath = currentPath.isEmpty ? component : currentPath + "/" + component
                if let existing = node.folders[currentPath] {
                    node = existing
                } else {
                    let created = VCSFileTreeNode()
                    node.folders[currentPath] = created
                    node = created
                }
            }

            node.files.append(file)
        }

        return root
    }

    private struct CompactedFolder {
        let path: String
        let name: String
        let node: VCSFileTreeNode
    }

    private static func compactedFolder(
        startPath: String,
        startName: String,
        node: VCSFileTreeNode
    ) -> CompactedFolder {
        guard node.files.isEmpty, node.folders.count == 1, let (nextPath, nextNode) = node.folders.first else {
            return CompactedFolder(path: startPath, name: startName, node: node)
        }
        let nextName = (nextPath as NSString).lastPathComponent
        return compactedFolder(startPath: nextPath, startName: startName + "/" + nextName, node: nextNode)
    }

    private static func flattenRows(node: VCSFileTreeNode, depth: Int, expandedFolders: Set<String>) -> [Row] {
        var result: [Row] = []

        let sortedFolderPaths = node.folders.keys.sorted {
            let lhs = ($0 as NSString).lastPathComponent
            let rhs = ($1 as NSString).lastPathComponent
            return lhs.localizedCaseInsensitiveCompare(rhs) == .orderedAscending
        }

        for folderPath in sortedFolderPaths {
            guard let child = node.folders[folderPath] else { continue }
            let compacted = compactedFolder(
                startPath: folderPath,
                startName: (folderPath as NSString).lastPathComponent,
                node: child
            )
            result.append(.folder(Folder(
                path: compacted.path,
                name: compacted.name,
                depth: depth,
                fileCount: compacted.node.totalFileCount
            )))

            guard expandedFolders.contains(compacted.path) else { continue }
            result.append(contentsOf: flattenRows(node: compacted.node, depth: depth + 1, expandedFolders: expandedFolders))
        }

        let sortedFiles = node.files.sorted {
            let lhs = ($0.path as NSString).lastPathComponent
            let rhs = ($1.path as NSString).lastPathComponent
            return lhs.localizedCaseInsensitiveCompare(rhs) == .orderedAscending
        }

        for file in sortedFiles {
            result.append(.file(file, depth: depth))
        }

        return result
    }
}

private final class VCSFileTreeNode {
    var folders: [String: VCSFileTreeNode] = [:]
    var files: [GitStatusFile] = []

    var totalFileCount: Int {
        files.count + folders.values.reduce(0) { $0 + $1.totalFileCount }
    }
}
