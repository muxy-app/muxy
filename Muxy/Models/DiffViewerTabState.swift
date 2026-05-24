import Foundation

@MainActor
@Observable
final class DiffViewerTabState: Identifiable {
    struct CommitSource: Equatable {
        let hash: String
        let subject: String
        let webURL: URL?

        var shortHash: String {
            String(hash.prefix(7))
        }
    }

    struct PullRequestSource: Equatable {
        let number: Int
        let title: String
        let baseRef: String
        let headRef: String
        let webURL: URL?
    }

    enum Source: Equatable {
        case workingTree
        case commit(CommitSource)
        case range(baseRef: String, headRef: String, title: String)
        case pullRequest(PullRequestSource)

        var displayTitle: String {
            switch self {
            case .workingTree:
                "Git Diff"
            case let .commit(commit):
                "Commit \(commit.shortHash) Diff"
            case let .range(_, _, title):
                title
            case let .pullRequest(pullRequest):
                "PR #\(pullRequest.number) Diff"
            }
        }

        var link: (title: String, url: URL)? {
            switch self {
            case .workingTree,
                 .range:
                nil
            case let .commit(commit):
                commit.webURL.map { ("Commit \(commit.shortHash)", $0) }
            case let .pullRequest(pullRequest):
                pullRequest.webURL.map { ("PR #\(pullRequest.number)", $0) }
            }
        }
    }

    let id = UUID()
    let vcs: VCSTabState
    let projectPath: String
    var source: Source
    var mode: VCSTabState.ViewMode
    var selectedFilePath: String?
    var selectedIsStaged = false
    var wordWrap = false
    var fontSize: CGFloat = 13
    var scrollRequestVersion = 0
    var sidebarScrollRequestVersion = 0
    var collapsedCacheKeys: Set<String> = []
    var manuallyLoadedCacheKeys: Set<String> = []
    var activeCacheKey: String?
    var sourceFiles: [GitStatusFile] = []
    var isLoadingFiles = false
    var filesError: String?
    let diffCache = DiffCache()
    private let git = GitRepositoryService()

    var displayTitle: String {
        source.displayTitle
    }

    var files: [GitStatusFile] {
        source == .workingTree ? vcs.files : sourceFiles
    }

    var stagedFiles: [GitStatusFile] {
        source == .workingTree ? vcs.stagedFiles : []
    }

    var unstagedFiles: [GitStatusFile] {
        source == .workingTree ? vcs.unstagedFiles : sourceFiles
    }

    var selectedDisplayTitle: String {
        guard let selectedFilePath else { return "No file selected" }
        return (selectedFilePath as NSString).lastPathComponent
    }

    var selectedCacheKey: String? {
        guard let selectedFilePath else { return nil }
        return Self.cacheKey(filePath: selectedFilePath, isStaged: selectedIsStaged)
    }

    init(vcs: VCSTabState, filePath: String? = nil, isStaged: Bool = false, source: Source = .workingTree) {
        self.vcs = vcs
        self.source = source
        projectPath = vcs.projectPath
        mode = vcs.mode
        selectInitialFile(filePath: filePath, isStaged: isStaged)
    }

    func refresh(forceFull: Bool) {
        if source == .workingTree {
            loadAllDiffs(forceFull: forceFull)
        } else {
            loadSourceFiles(forceFull: forceFull)
        }
    }

    func setSource(_ source: Source, filePath: String? = nil, isStaged: Bool = false) {
        self.source = source
        selectedFilePath = nil
        selectedIsStaged = isStaged
        activeCacheKey = nil
        collapsedCacheKeys.removeAll()
        manuallyLoadedCacheKeys.removeAll()
        diffCache.clearAll()
        sourceFiles.removeAll()
        if source == .workingTree {
            selectInitialFile(filePath: filePath, isStaged: isStaged)
        } else {
            loadSourceFiles(forceFull: false)
        }
    }

    func loadFullDiff(filePath: String, isStaged: Bool) {
        let cacheKey = Self.cacheKey(filePath: filePath, isStaged: isStaged)
        manuallyLoadedCacheKeys.insert(cacheKey)
        collapsedCacheKeys.remove(cacheKey)
        loadDiff(filePath: filePath, isStaged: isStaged, forceFull: true)
    }

    func select(filePath: String, isStaged: Bool) {
        guard selectedFilePath != filePath || selectedIsStaged != isStaged else {
            scrollRequestVersion &+= 1
            loadSelectedDiff(forceFull: false)
            return
        }
        selectedFilePath = filePath
        selectedIsStaged = isStaged
        activeCacheKey = Self.cacheKey(filePath: filePath, isStaged: isStaged)
        scrollRequestVersion &+= 1
        loadSelectedDiff(forceFull: false)
    }

    func activateFromDiffScroll(cacheKey: String?) {
        guard activeCacheKey != cacheKey else { return }
        activeCacheKey = cacheKey
        sidebarScrollRequestVersion &+= 1
    }

    func loadAllDiffs(forceFull: Bool = false) {
        if source != .workingTree, sourceFiles.isEmpty {
            loadSourceFiles(forceFull: forceFull)
            return
        }
        for file in stagedFiles {
            loadDiff(filePath: file.path, isStaged: true, forceFull: forceFull)
        }
        for file in unstagedFiles {
            loadDiff(filePath: file.path, isStaged: false, forceFull: forceFull)
        }
    }

    func adjustFontSize(by delta: CGFloat) {
        fontSize = min(28, max(9, fontSize + delta))
    }

    func resetFontSize() {
        fontSize = 13
    }

    func isCollapsed(filePath: String, isStaged: Bool) -> Bool {
        collapsedCacheKeys.contains(Self.cacheKey(filePath: filePath, isStaged: isStaged))
    }

    func toggleCollapsed(filePath: String, isStaged: Bool) {
        let cacheKey = Self.cacheKey(filePath: filePath, isStaged: isStaged)
        if isLargeUnloadedDiff(cacheKey) {
            loadFullDiff(filePath: filePath, isStaged: isStaged)
            return
        }
        if collapsedCacheKeys.contains(cacheKey) {
            collapsedCacheKeys.remove(cacheKey)
        } else {
            collapsedCacheKeys.insert(cacheKey)
        }
    }

    func collapseAll() {
        collapsedCacheKeys = allCacheKeys
    }

    func expandAll() {
        collapsedCacheKeys = Set(allCacheKeys.filter(isLargeUnloadedDiff))
    }

    func reconcileLargeDiffCollapse() {
        collapsedCacheKeys.formUnion(allCacheKeys.filter(isLargeUnloadedDiff))
    }

    func reconcileSelection() {
        if let selectedFilePath, contains(filePath: selectedFilePath, isStaged: selectedIsStaged) {
            loadSelectedDiff(forceFull: false)
            return
        }
        if let selectedFilePath, contains(filePath: selectedFilePath, isStaged: !selectedIsStaged) {
            select(filePath: selectedFilePath, isStaged: !selectedIsStaged)
            return
        }
        if let first = stagedFiles.first {
            select(filePath: first.path, isStaged: true)
            return
        }
        if let first = unstagedFiles.first {
            select(filePath: first.path, isStaged: false)
            return
        }
        selectedFilePath = nil
    }

    func diff() -> DiffCache.LoadedDiff? {
        guard let selectedCacheKey else { return nil }
        return activeDiffCache.diff(for: selectedCacheKey)
    }

    func isLoading() -> Bool {
        guard let selectedCacheKey else { return false }
        return activeDiffCache.isLoading(selectedCacheKey)
    }

    func error() -> String? {
        guard let selectedCacheKey else { return nil }
        return activeDiffCache.error(for: selectedCacheKey)
    }

    private func selectInitialFile(filePath: String?, isStaged: Bool) {
        if let filePath, contains(filePath: filePath, isStaged: isStaged) {
            selectedFilePath = filePath
            selectedIsStaged = isStaged
            loadSelectedDiff(forceFull: false)
            return
        }
        reconcileSelection()
    }

    private func loadSelectedDiff(forceFull: Bool) {
        guard let selectedFilePath else { return }
        loadDiff(filePath: selectedFilePath, isStaged: selectedIsStaged, forceFull: forceFull)
    }

    private func loadDiff(filePath: String, isStaged: Bool, forceFull: Bool) {
        if source != .workingTree {
            loadSourceDiff(filePath: filePath, isStaged: isStaged, forceFull: forceFull)
            return
        }
        vcs.loadDiffWithHints(
            filePath: filePath,
            hints: diffHints(filePath: filePath, isStaged: isStaged),
            cacheKey: Self.cacheKey(filePath: filePath, isStaged: isStaged),
            pinnedPaths: allCacheKeys,
            forceFull: forceFull
        )
    }

    private var allCacheKeys: Set<String> {
        Set(stagedFiles.map { Self.cacheKey(filePath: $0.path, isStaged: true) } +
            unstagedFiles.map { Self.cacheKey(filePath: $0.path, isStaged: false) })
    }

    private func contains(filePath: String, isStaged: Bool) -> Bool {
        if isStaged {
            return stagedFiles.contains { $0.path == filePath }
        }
        return unstagedFiles.contains { $0.path == filePath }
    }

    private func diffHints(filePath: String, isStaged: Bool) -> GitRepositoryService.DiffHints {
        guard let file = files.first(where: { $0.path == filePath }) else {
            return GitRepositoryService.DiffHints(hasStaged: isStaged, hasUnstaged: !isStaged, isUntrackedOrNew: false)
        }
        let untrackedOrNew = (file.xStatus == "?" && file.yStatus == "?") || (!isStaged && file.xStatus == "A")
        if isStaged {
            return GitRepositoryService.DiffHints(hasStaged: true, hasUnstaged: false, isUntrackedOrNew: untrackedOrNew)
        }
        return GitRepositoryService.DiffHints(hasStaged: false, hasUnstaged: !untrackedOrNew, isUntrackedOrNew: untrackedOrNew)
    }

    private func isLargeUnloadedDiff(_ cacheKey: String) -> Bool {
        activeDiffCache.diff(for: cacheKey)?.truncated == true && !manuallyLoadedCacheKeys.contains(cacheKey)
    }

    private var activeDiffCache: DiffCache {
        source == .workingTree ? vcs.diffCache : diffCache
    }

    private func loadSourceFiles(forceFull: Bool) {
        guard source != .workingTree else { return }
        isLoadingFiles = true
        filesError = nil
        let source = source
        Task { [weak self] in
            guard let self else { return }
            do {
                let files = try await sourceFiles(for: source)
                guard !Task.isCancelled else { return }
                sourceFiles = files
                isLoadingFiles = false
                reconcileSelection()
                loadAllDiffs(forceFull: forceFull)
            } catch {
                guard !Task.isCancelled else { return }
                sourceFiles = []
                isLoadingFiles = false
                filesError = (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
            }
        }
    }

    private func sourceFiles(for source: Source) async throws -> [GitStatusFile] {
        switch source {
        case .workingTree:
            vcs.files
        case let .commit(commit):
            try await git.changedFiles(repoPath: projectPath, commit: commit.hash)
        case let .range(baseRef, headRef, _):
            try await git.changedFiles(
                repoPath: projectPath,
                range: GitRepositoryService.DiffRange(baseRef: baseRef, headRef: headRef)
            )
        case let .pullRequest(pullRequest):
            try await git.changedFiles(
                repoPath: projectPath,
                range: GitRepositoryService.DiffRange(baseRef: pullRequest.baseRef, headRef: pullRequest.headRef)
            )
        }
    }

    private func loadSourceDiff(filePath: String, isStaged: Bool, forceFull: Bool) {
        let cacheKey = Self.cacheKey(filePath: filePath, isStaged: isStaged)
        if !forceFull, diffCache.hasDiff(for: cacheKey) {
            diffCache.touch(cacheKey)
            return
        }
        if !forceFull, diffCache.isLoading(cacheKey) { return }
        if forceFull { diffCache.cancelLoad(for: cacheKey) }
        diffCache.markLoading(cacheKey)
        let source = source
        let lineLimit = forceFull ? nil : DiffLoader.previewLineLimit
        let task = Task { [weak self] in
            guard let self else { return }
            do {
                let result = try await sourceDiff(filePath: filePath, source: source, lineLimit: lineLimit)
                guard !Task.isCancelled else { return }
                diffCache.store(
                    DiffCache.LoadedDiff(
                        rows: result.rows,
                        additions: result.additions,
                        deletions: result.deletions,
                        truncated: result.truncated
                    ),
                    for: cacheKey,
                    pinnedPaths: allCacheKeys
                )
            } catch {
                guard !Task.isCancelled else { return }
                diffCache.storeError((error as? LocalizedError)?.errorDescription ?? error.localizedDescription, for: cacheKey)
            }
        }
        diffCache.registerTask(task, for: cacheKey)
    }

    private func sourceDiff(
        filePath: String,
        source: Source,
        lineLimit: Int?
    ) async throws -> GitRepositoryService.PatchAndCompareResult {
        switch source {
        case .workingTree:
            try await git.patchAndCompare(repoPath: projectPath, filePath: filePath, lineLimit: lineLimit)
        case let .commit(commit):
            try await git.patchAndCompare(repoPath: projectPath, filePath: filePath, commit: commit.hash, lineLimit: lineLimit)
        case let .range(baseRef, headRef, _):
            try await git.patchAndCompare(
                repoPath: projectPath,
                filePath: filePath,
                range: GitRepositoryService.DiffRange(baseRef: baseRef, headRef: headRef),
                lineLimit: lineLimit
            )
        case let .pullRequest(pullRequest):
            try await git.patchAndCompare(
                repoPath: projectPath,
                filePath: filePath,
                range: GitRepositoryService.DiffRange(baseRef: pullRequest.baseRef, headRef: pullRequest.headRef),
                lineLimit: lineLimit
            )
        }
    }

    static func cacheKey(filePath: String, isStaged: Bool) -> String {
        "\(isStaged ? "staged" : "unstaged"):\(filePath)"
    }
}
