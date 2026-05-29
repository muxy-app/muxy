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
        let baseRef: String?
        let headRef: String?
        let baseBranch: String?
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
    var fontSize: CGFloat = DiffViewerTabState.loadPersistedFontSize()
    var scrollRequestVersion = 0
    var sidebarScrollRequestVersion = 0
    var collapsedCacheKeys: Set<String> = []
    var manuallyLoadedCacheKeys: Set<String> = []
    var activeCacheKey: String?
    var sourceFiles: [GitStatusFile] = []
    var isLoadingFiles = false
    var filesError: String?
    var sessionComments: [DiffInlineComment] = []
    var activeComposer: DiffCommentComposerTarget?
    var composerLineCount = 1
    var commentAuthor = "You"
    var commentAuthorAvatarURL: URL?
    weak var tabArea: TabArea?
    let diffCache = DiffCache()
    private let git = GitRepositoryService()
    private var commentAuthorTask: Task<Void, Never>?
    private var sourceFilesTask: Task<Void, Never>?
    private var pendingScrollCacheKey: String?
    static let fontSizeDefaultsKey = "muxy.diffViewer.fontSize"
    private static let defaultFontSize: CGFloat = 13
    private static let minFontSize: CGFloat = 9
    private static let maxFontSize: CGFloat = 28
    private static var instances: [WeakDiffViewerTabState] = []

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
        Self.register(self)
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
        cancelSourceLoads()
        self.source = source
        selectedFilePath = nil
        selectedIsStaged = isStaged
        activeCacheKey = nil
        pendingScrollCacheKey = nil
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

    func prepareForClose() {
        cancelSourceLoads()
        commentAuthorTask?.cancel()
        diffCache.clearAll()
        vcs.diffCache.evict { key in
            key.hasPrefix("staged:") || key.hasPrefix("unstaged:")
        }
    }

    var isPR: Bool {
        if case .pullRequest = source { return true }
        return false
    }

    var hasUnsentSessionComments: Bool {
        sessionComments.contains { $0.submissionState == .draft }
    }

    func comments(forCacheKey cacheKey: String) -> [DiffInlineComment] {
        sessionComments.filter { $0.cacheKey == cacheKey }
    }

    func loadCommentAuthor() {
        guard commentAuthorTask == nil else { return }
        let projectPath = projectPath
        commentAuthorTask = Task { [weak self] in
            guard let user = await GitRepositoryService().currentGitHubUser(repoPath: projectPath) else { return }
            guard !Task.isCancelled else { return }
            self?.commentAuthor = user.login
            self?.commentAuthorAvatarURL = user.avatarURL
        }
    }

    func beginComposer(cacheKey: String, filePath: String, side: DiffCommentSide, startLine: Int, endLine: Int) {
        composerLineCount = 1
        activeComposer = DiffCommentComposerTarget(
            cacheKey: cacheKey,
            filePath: filePath,
            side: side,
            startLine: min(startLine, endLine),
            endLine: max(startLine, endLine)
        )
    }

    func cancelComposer() {
        activeComposer = nil
        composerLineCount = 1
    }

    func submitComment(body: String) {
        let trimmed = body.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, let target = activeComposer else { return }
        let comment = DiffInlineComment(
            cacheKey: target.cacheKey,
            filePath: target.filePath,
            side: target.side,
            startLine: target.startLine,
            endLine: target.endLine,
            body: trimmed,
            author: commentAuthor,
            createdAt: Date(),
            submissionState: isPR ? .posting : .draft
        )
        activeComposer = nil
        if isPR {
            postToPullRequest(comment)
        } else {
            sessionComments.append(comment)
        }
    }

    func removeComment(id: UUID) {
        sessionComments.removeAll { $0.id == id }
    }

    private func postToPullRequest(_ comment: DiffInlineComment) {
        guard case let .pullRequest(pullRequest) = source,
              let headRef = pullRequest.headRef
        else { return }
        sessionComments.append(comment)
        let projectPath = projectPath
        Task { [weak self] in
            let result = await GitRepositoryService().postPullRequestReviewComment(
                GitRepositoryService.PRCommentRequest(
                    repoPath: projectPath,
                    number: pullRequest.number,
                    commit: headRef,
                    path: comment.filePath,
                    line: comment.endLine,
                    side: comment.side == .old ? .left : .right,
                    body: comment.body
                )
            )
            guard let self else { return }
            switch result {
            case .success:
                sessionComments.removeAll { $0.id == comment.id }
            case let .failure(message):
                updateSubmissionState(id: comment.id, state: .failed(message))
            }
        }
    }

    private func updateSubmissionState(id: UUID, state: DiffCommentSubmissionState) {
        guard let index = sessionComments.firstIndex(where: { $0.id == id }) else { return }
        sessionComments[index].submissionState = state
    }

    func runByAgent(commentID: UUID, provider: AIAssistantProvider) {
        guard let comment = sessionComments.first(where: { $0.id == commentID }) else { return }
        launchAgent(with: [comment], provider: provider, title: "\(provider.displayName): \(comment.filePath)")
    }

    func runByAgent(target: DiffCommentComposerTarget, body: String, provider: AIAssistantProvider) {
        let trimmed = body.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        let comment = DiffInlineComment(
            cacheKey: target.cacheKey,
            filePath: target.filePath,
            side: target.side,
            startLine: target.startLine,
            endLine: target.endLine,
            body: trimmed,
            author: commentAuthor,
            createdAt: Date()
        )
        activeComposer = nil
        sessionComments.append(comment)
        launchAgent(with: [comment], provider: provider, title: "\(provider.displayName): \(comment.filePath)")
    }

    func runAllByAgent(provider: AIAssistantProvider) {
        let comments = sessionComments.filter { $0.submissionState == .draft }
        guard !comments.isEmpty else { return }
        launchAgent(with: comments, provider: provider, title: "\(provider.displayName): \(comments.count) comments")
    }

    private func launchAgent(with comments: [DiffInlineComment], provider: AIAssistantProvider, title: String) {
        guard let tabArea else { return }
        let prompt = DiffAgentPrompt.build(comments: comments, rowsProvider: rowsForComment)
        let command = DiffAgentLauncher.command(for: provider, settings: AIAssistantSettings.snapshot(), prompt: prompt)
        guard let command else { return }
        tabArea.createCommandTab(name: title, command: command, closesOnCommandExit: false)
    }

    private func rowsForComment(_ comment: DiffInlineComment) -> [DiffDisplayRow] {
        activeDiffCache.diff(for: comment.cacheKey)?.rows ?? []
    }

    func reconcileCommentAnchors() {
        for index in sessionComments.indices {
            let comment = sessionComments[index]
            guard comment.submissionState == .draft || comment.submissionState == .outdated else { continue }
            let rows = activeDiffCache.diff(for: comment.cacheKey)?.rows ?? []
            guard !rows.isEmpty else { continue }
            let resolved = DiffCommentAnchor.resolveDisplayRowIndex(
                side: comment.side,
                line: comment.startLine,
                in: rows
            )
            sessionComments[index].submissionState = resolved == nil ? .outdated : .draft
        }
    }

    func loadFullDiff(filePath: String, isStaged: Bool) {
        let cacheKey = Self.cacheKey(filePath: filePath, isStaged: isStaged)
        manuallyLoadedCacheKeys.insert(cacheKey)
        collapsedCacheKeys.remove(cacheKey)
        loadDiff(filePath: filePath, isStaged: isStaged, forceFull: true)
    }

    func loadDiff(filePath: String, isStaged: Bool) {
        loadDiff(filePath: filePath, isStaged: isStaged, forceFull: false)
    }

    func select(filePath: String, isStaged: Bool) {
        let cacheKey = Self.cacheKey(filePath: filePath, isStaged: isStaged)
        guard selectedFilePath != filePath || selectedIsStaged != isStaged else {
            activeCacheKey = cacheKey
            pendingScrollCacheKey = cacheKey
            scrollRequestVersion &+= 1
            loadSelectedDiff(forceFull: false)
            return
        }
        selectedFilePath = filePath
        selectedIsStaged = isStaged
        activeCacheKey = cacheKey
        pendingScrollCacheKey = cacheKey
        scrollRequestVersion &+= 1
        loadSelectedDiff(forceFull: false)
    }

    func activateFromDiffScroll(cacheKey: String?) {
        if let pendingScrollCacheKey {
            guard cacheKey == pendingScrollCacheKey else { return }
            self.pendingScrollCacheKey = nil
        }
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
        Self.storeFontSize(fontSize + delta)
    }

    func resetFontSize() {
        Self.storeFontSize(Self.defaultFontSize)
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
        collapsedCacheKeys.formUnion(allCacheKeys.filter { cacheKey in
            isLargeUnloadedDiff(cacheKey) || isEstimatedLargeDiff(cacheKey)
        })
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
        return DiffHintPolicy.hints(file: file, isStaged: isStaged)
    }

    private func isLargeUnloadedDiff(_ cacheKey: String) -> Bool {
        activeDiffCache.diff(for: cacheKey)?.truncated == true && !manuallyLoadedCacheKeys.contains(cacheKey)
    }

    private func isEstimatedLargeDiff(_ cacheKey: String) -> Bool {
        guard !activeDiffCache.hasDiff(for: cacheKey) else { return false }
        let isStaged = cacheKey.hasPrefix("staged:")
        guard let file = files.first(where: { Self.cacheKey(filePath: $0.path, isStaged: isStaged) == cacheKey })
        else {
            return false
        }
        return DiffCollapsePolicy.shouldCollapseByDefault(file, isStaged: isStaged)
    }

    private var activeDiffCache: DiffCache {
        source == .workingTree ? vcs.diffCache : diffCache
    }

    private static func register(_ state: DiffViewerTabState) {
        instances.removeAll { $0.value == nil }
        instances.append(WeakDiffViewerTabState(value: state))
    }

    private static func loadPersistedFontSize() -> CGFloat {
        let defaults = UserDefaults.standard
        guard defaults.object(forKey: fontSizeDefaultsKey) != nil else { return defaultFontSize }
        return clampedFontSize(CGFloat(defaults.double(forKey: fontSizeDefaultsKey)))
    }

    private static func storeFontSize(_ fontSize: CGFloat) {
        let fontSize = clampedFontSize(fontSize)
        UserDefaults.standard.set(Double(fontSize), forKey: fontSizeDefaultsKey)
        instances.removeAll { $0.value == nil }
        for instance in instances {
            instance.value?.fontSize = fontSize
        }
    }

    private static func clampedFontSize(_ fontSize: CGFloat) -> CGFloat {
        min(maxFontSize, max(minFontSize, fontSize))
    }

    private func loadSourceFiles(forceFull: Bool) {
        guard source != .workingTree else { return }
        sourceFilesTask?.cancel()
        diffCache.cancelAndClearLoading()
        isLoadingFiles = true
        filesError = nil
        let source = source
        let task = Task { [weak self] in
            guard let self else { return }
            do {
                let resolvedSource = try await resolvedSource(source)
                guard !Task.isCancelled else { return }
                self.source = resolvedSource
                let files = try await sourceFiles(for: resolvedSource)
                guard !Task.isCancelled else { return }
                sourceFiles = files
                isLoadingFiles = false
                reconcileLargeDiffCollapse()
                reconcileSelection()
                loadAllDiffs(forceFull: forceFull)
            } catch {
                guard !Task.isCancelled else { return }
                sourceFiles = []
                isLoadingFiles = false
                filesError = (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
            }
        }
        sourceFilesTask = task
    }

    private func resolvedSource(_ source: Source) async throws -> Source {
        guard case let .pullRequest(pullRequest) = source,
              pullRequest.baseRef == nil || pullRequest.headRef == nil
        else { return source }

        let remote = await git.githubRemoteName(repoPath: projectPath) ?? "origin"
        let baseRef = pullRequest.baseRef ?? "refs/remotes/\(remote)/\(pullRequest.baseBranch ?? "main")"
        let headRef = try await git.fetchPullRequestDiffHead(
            repoPath: projectPath,
            number: pullRequest.number,
            remote: remote
        )
        return .pullRequest(PullRequestSource(
            number: pullRequest.number,
            title: pullRequest.title,
            baseRef: baseRef,
            headRef: headRef,
            baseBranch: pullRequest.baseBranch,
            webURL: pullRequest.webURL
        ))
    }

    private func sourceFiles(for source: Source) async throws -> [GitStatusFile] {
        switch source {
        case .workingTree:
            return vcs.files
        case let .commit(commit):
            return try await git.changedFiles(repoPath: projectPath, commit: commit.hash)
        case let .range(baseRef, headRef, _):
            return try await git.changedFiles(
                repoPath: projectPath,
                range: GitRepositoryService.DiffRange(baseRef: baseRef, headRef: headRef)
            )
        case let .pullRequest(pullRequest):
            guard let baseRef = pullRequest.baseRef, let headRef = pullRequest.headRef else { return [] }
            return try await git.changedFiles(
                repoPath: projectPath,
                range: GitRepositoryService.DiffRange(baseRef: baseRef, headRef: headRef)
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

    private func cancelSourceLoads() {
        sourceFilesTask?.cancel()
        sourceFilesTask = nil
        diffCache.cancelAndClearLoading()
        isLoadingFiles = false
    }

    private func sourceDiff(
        filePath: String,
        source: Source,
        lineLimit: Int?
    ) async throws -> GitRepositoryService.PatchAndCompareResult {
        try await DiffLoadGate.shared.acquire()
        defer { Task { await DiffLoadGate.shared.release() } }
        try Task.checkCancellation()
        switch source {
        case .workingTree:
            return try await git.patchAndCompare(repoPath: projectPath, filePath: filePath, lineLimit: lineLimit)
        case let .commit(commit):
            return try await git.patchAndCompare(
                repoPath: projectPath,
                filePath: filePath,
                commit: commit.hash,
                lineLimit: lineLimit
            )
        case let .range(baseRef, headRef, _):
            return try await git.patchAndCompare(
                repoPath: projectPath,
                filePath: filePath,
                range: GitRepositoryService.DiffRange(baseRef: baseRef, headRef: headRef),
                lineLimit: lineLimit
            )
        case let .pullRequest(pullRequest):
            guard let baseRef = pullRequest.baseRef, let headRef = pullRequest.headRef else {
                return GitRepositoryService.PatchAndCompareResult(rows: [], truncated: false, additions: 0, deletions: 0)
            }
            return try await git.patchAndCompare(
                repoPath: projectPath,
                filePath: filePath,
                range: GitRepositoryService.DiffRange(baseRef: baseRef, headRef: headRef),
                lineLimit: lineLimit
            )
        }
    }

    static func cacheKey(filePath: String, isStaged: Bool) -> String {
        "\(isStaged ? "staged" : "unstaged"):\(filePath)"
    }
}

private struct WeakDiffViewerTabState {
    weak var value: DiffViewerTabState?
}

actor DiffLoadGate {
    static let shared = DiffLoadGate(limit: 4)

    private final class Waiter {
        let id: UUID
        var continuation: CheckedContinuation<Bool, Never>?

        init(id: UUID, continuation: CheckedContinuation<Bool, Never>) {
            self.id = id
            self.continuation = continuation
        }
    }

    private let limit: Int
    private var active = 0
    private var waiters: [Waiter] = []

    init(limit: Int) {
        self.limit = limit
    }

    func acquire() async throws {
        try Task.checkCancellation()
        if active < limit {
            active += 1
            return
        }
        let id = UUID()
        let granted = await withTaskCancellationHandler {
            await withCheckedContinuation { (continuation: CheckedContinuation<Bool, Never>) in
                waiters.append(Waiter(id: id, continuation: continuation))
            }
        } onCancel: {
            Task { await self.cancelWaiter(id: id) }
        }
        if !granted {
            throw CancellationError()
        }
    }

    func release() {
        if let next = waiters.first, let continuation = next.continuation {
            waiters.removeFirst()
            next.continuation = nil
            continuation.resume(returning: true)
            return
        }
        active -= 1
    }

    private func cancelWaiter(id: UUID) {
        guard let index = waiters.firstIndex(where: { $0.id == id }) else { return }
        let waiter = waiters[index]
        guard let continuation = waiter.continuation else { return }
        waiter.continuation = nil
        waiters.remove(at: index)
        continuation.resume(returning: false)
    }
}
