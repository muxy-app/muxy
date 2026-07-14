import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "TabFocusedRepositoryState")

@MainActor
@Observable
final class TabFocusedRepositoryState {
    enum PullRequestFetchState: Equatable {
        case loading
        case noPullRequest
        case unavailable
        case found(GitRepositoryService.PRInfo)
    }

    struct PullRequestIdentity: Equatable {
        let repositoryKey: String
        let branch: String
        let headOID: String
    }

    static let notificationOriginKey = "muxy.repositoryToolbar.origin"
    static let notificationOriginID = "tabFocusedRepositoryToolbar"

    private(set) var summary: GitRepositorySummary?
    private(set) var branches: [String] = []
    private(set) var changesSnapshot = RepositoryChangesSnapshot.empty
    private(set) var untrackedLineStats: [String: Int] = [:]
    private(set) var untrackedLineStatsSummary = RepositoryChangesSnapshot.empty.totalLineStats
    private(set) var hasLoadedChanges = false
    private(set) var pullRequestState: PullRequestFetchState = .loading
    private(set) var isLoadingSummary = false
    private(set) var isLoadingBranches = false
    private(set) var isLoadingChanges = false
    private(set) var isMutatingChanges = false
    private(set) var isRefreshingPullRequest = false
    private(set) var isSwitchingBranch = false
    private(set) var branchBeingDeleted: String?
    private(set) var isMergingPullRequest = false
    private(set) var isClosingPullRequest = false
    private(set) var isUpdatingPullRequestBranch = false
    private(set) var summaryError: String?
    private(set) var changesError: String?

    @ObservationIgnored private var activeRepository: ActiveRepository?
    @ObservationIgnored private var fileSystemWatcher: FileSystemWatcher?
    @ObservationIgnored private var gitDirectoryFileSystemWatcher: FileSystemWatcher?
    @ObservationIgnored private var fileRefreshTask: Task<Void, Never>?
    @ObservationIgnored private var summaryRevision = 0
    @ObservationIgnored private var branchesRevision = 0
    @ObservationIgnored private var changesRevision = 0
    @ObservationIgnored private var workingTreeRefreshRevision = 0
    @ObservationIgnored private var pullRequestRevision = 0
    @ObservationIgnored private var pullRequestIdentity: PullRequestIdentity?
    @ObservationIgnored private var isChangesMonitoringEnabled = false
    @ObservationIgnored private var loadedUntrackedLineStats: Set<String> = []
    @ObservationIgnored private var loadingUntrackedLineStats: Set<String> = []

    var pullRequest: GitRepositoryService.PRInfo? {
        guard case let .found(info) = pullRequestState else { return nil }
        return info
    }

    var isMutatingBranches: Bool {
        isSwitchingBranch || branchBeingDeleted != nil
    }

    func activate(repoPath: String, context: WorkspaceContext) async {
        let repository = ActiveRepository(path: repoPath, context: context)
        if activeRepository != repository {
            reset(for: repository)
        }
        guard await refreshSummary(refreshPullRequestOnHeadChange: false) else { return }
        await refreshPullRequest(forceFresh: false)
    }

    func deactivate() {
        activeRepository = nil
        fileSystemWatcher = nil
        gitDirectoryFileSystemWatcher = nil
        fileRefreshTask?.cancel()
        fileRefreshTask = nil
        summaryRevision += 1
        branchesRevision += 1
        changesRevision += 1
        workingTreeRefreshRevision += 1
        pullRequestRevision += 1
        summary = nil
        branches = []
        resetChangesPresentation(hasLoaded: false)
        pullRequestState = .loading
        pullRequestIdentity = nil
        summaryError = nil
        changesError = nil
        isChangesMonitoringEnabled = false
        resetTransientState()
    }

    func refreshRepositoryDetails() async {
        guard await refreshSummary() else { return }
        await loadBranches()
    }

    func refreshWorkingTreeDetails() async {
        workingTreeRefreshRevision += 1
        let refreshRevision = workingTreeRefreshRevision
        changesRevision += 1
        let invalidatedChangesRevision = changesRevision
        let requestedSummaryRevision = summaryRevision + 1
        isChangesMonitoringEnabled = true
        isLoadingChanges = true
        guard await refreshSummary(), !Task.isCancelled else {
            if refreshRevision == workingTreeRefreshRevision,
               invalidatedChangesRevision == changesRevision,
               requestedSummaryRevision == summaryRevision
            {
                isLoadingChanges = false
            }
            return
        }
        guard refreshRevision == workingTreeRefreshRevision else { return }
        await loadChanges()
    }

    func setChangesMonitoring(_ isEnabled: Bool) {
        isChangesMonitoringEnabled = isEnabled
    }

    func retryRepository() async {
        guard await refreshSummary(refreshPullRequestOnHeadChange: false) else { return }
        await refreshPullRequest(forceFresh: true)
    }

    func refreshAfterAppActivation() async {
        guard await refreshSummary(refreshPullRequestOnHeadChange: false) else { return }
        if isChangesMonitoringEnabled {
            await loadChanges()
        }
        await refreshPullRequest(forceFresh: false)
    }

    func refreshFromExternalChange() async {
        guard await refreshSummary(refreshPullRequestOnHeadChange: false) else { return }
        if isChangesMonitoringEnabled {
            await loadChanges()
        }
        await refreshPullRequest(forceFresh: true)
    }

    func loadChanges() async {
        changesRevision += 1
        let revision = changesRevision
        guard let repository = activeRepository, summary?.isDirty == true else {
            resetChangesPresentation(hasLoaded: true)
            changesError = nil
            isLoadingChanges = false
            return
        }
        isLoadingChanges = true
        changesError = nil
        defer {
            if revision == changesRevision {
                isLoadingChanges = false
            }
        }
        do {
            let files = try await repository.service.changedFiles(
                repoPath: repository.path,
                includeUntrackedLineCounts: false
            )
            let snapshot = await RepositoryChangesPresentation.loadSnapshot(files)
            guard !Task.isCancelled,
                  repository == activeRepository,
                  revision == changesRevision
            else { return }
            loadedUntrackedLineStats = []
            loadingUntrackedLineStats = []
            changesSnapshot = snapshot
            untrackedLineStats = [:]
            untrackedLineStatsSummary = RepositoryChangesSnapshot.empty.totalLineStats
            hasLoadedChanges = true
        } catch {
            guard !Task.isCancelled else { return }
            guard repository == activeRepository, revision == changesRevision else { return }
            resetChangesPresentation(hasLoaded: false)
            changesError = error.localizedDescription
            ToastState.shared.show(title: "Failed to load changes", body: error.localizedDescription)
        }
    }

    func loadUntrackedLineStats(for file: GitStatusFile) async {
        guard file.isUntracked,
              file.additions == nil,
              let repository = activeRepository,
              !loadedUntrackedLineStats.contains(file.path),
              loadingUntrackedLineStats.insert(file.path).inserted
        else { return }
        let revision = changesRevision
        defer {
            if revision == changesRevision {
                loadingUntrackedLineStats.remove(file.path)
            }
        }
        do {
            let lineCount = try await repository.service.untrackedFileLineCount(
                repoPath: repository.path,
                path: file.path
            )
            guard !Task.isCancelled,
                  repository == activeRepository,
                  revision == changesRevision
            else { return }
            loadedUntrackedLineStats.insert(file.path)
            guard let lineCount else { return }
            let previousLineCount = untrackedLineStats.updateValue(lineCount, forKey: file.path) ?? 0
            untrackedLineStatsSummary = RepositoryChangesLineStats(
                additions: untrackedLineStatsSummary.additions + lineCount - previousLineCount,
                deletions: 0,
                hasKnownValues: true
            )
        } catch {
            guard !Task.isCancelled,
                  repository == activeRepository,
                  revision == changesRevision
            else { return }
            loadedUntrackedLineStats.insert(file.path)
            logger.debug("Failed to load line stats for \(file.path, privacy: .public): \(error.localizedDescription, privacy: .public)")
        }
    }

    func stage(_ file: GitStatusFile) async {
        await stage([file])
    }

    func stage(_ files: [GitStatusFile]) async {
        let paths = Array(Set(files.flatMap(\.relatedPaths))).sorted()
        guard !paths.isEmpty else { return }
        let failureTitle = files.count == 1 ? "Failed to stage \(files[0].path)" : "Failed to stage changes"
        await mutateChanges(failureTitle: failureTitle) { repository in
            try await repository.service.stageFiles(repoPath: repository.path, paths: paths)
        }
    }

    func unstage(_ file: GitStatusFile) async {
        await unstage([file])
    }

    func unstage(_ files: [GitStatusFile]) async {
        let paths = Array(Set(files.flatMap(\.relatedPaths))).sorted()
        guard !paths.isEmpty else { return }
        let failureTitle = files.count == 1 ? "Failed to unstage \(files[0].path)" : "Failed to unstage changes"
        await mutateChanges(failureTitle: failureTitle) { repository in
            try await repository.service.unstageFiles(repoPath: repository.path, paths: paths)
        }
    }

    func discard(_ file: GitStatusFile) async {
        guard let request = RepositoryChangesPresentation.discardRequest(file) else { return }
        await mutateChanges(failureTitle: "Failed to discard changes to \(file.path)") { repository in
            try await repository.service.discardFiles(
                repoPath: repository.path,
                paths: request.paths,
                untrackedPaths: request.untrackedPaths
            )
        }
    }

    func loadBranches() async {
        guard let repository = activeRepository else { return }
        branchesRevision += 1
        let revision = branchesRevision
        isLoadingBranches = true
        defer {
            if revision == branchesRevision {
                isLoadingBranches = false
            }
        }
        do {
            let loaded = try await repository.service.listBranches(repoPath: repository.path)
            guard repository == activeRepository, revision == branchesRevision else { return }
            branches = loaded
        } catch {
            guard repository == activeRepository, revision == branchesRevision else { return }
            branches = []
            ToastState.shared.show(title: "Failed to load branches", body: error.localizedDescription)
        }
    }

    func switchBranch(_ branch: String) async {
        guard branch != summary?.branch else { return }
        _ = await switchCurrentBranch(failureTitle: "Failed to switch branch") { repository in
            try await repository.service.switchBranch(repoPath: repository.path, branch: branch)
        }
    }

    func createAndSwitchBranch(_ name: String) async -> Bool {
        let branch = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !branch.isEmpty else { return false }
        let created = await switchCurrentBranch(failureTitle: "Failed to create branch") { repository in
            try await repository.service.createAndSwitchBranch(repoPath: repository.path, name: branch)
        }
        if created {
            ToastState.shared.show("Created branch \(branch)")
        }
        return created
    }

    func deleteBranch(_ branch: String) async -> Bool {
        guard let repository = activeRepository,
              branch != summary?.branch,
              !isMutatingBranches,
              !isMutatingChanges,
              !isPerformingPullRequestAction
        else { return false }
        branchBeingDeleted = branch
        defer {
            if repository == activeRepository {
                branchBeingDeleted = nil
            }
        }
        do {
            try await repository.service.deleteLocalBranch(repoPath: repository.path, branch: branch, force: true)
            guard repository == activeRepository else { return false }
            await loadBranches()
            ToastState.shared.show("Deleted branch \(branch)")
            postRepositoryChange(repository)
            return true
        } catch {
            guard repository == activeRepository else { return false }
            ToastState.shared.show(title: "Failed to delete branch \(branch)", body: error.localizedDescription)
            return false
        }
    }

    private func switchCurrentBranch(
        failureTitle: String,
        operation: (ActiveRepository) async throws -> Void
    ) async -> Bool {
        guard let repository = activeRepository,
              !isMutatingBranches,
              !isMutatingChanges,
              !isPerformingPullRequestAction
        else { return false }
        isSwitchingBranch = true
        defer {
            if repository == activeRepository {
                isSwitchingBranch = false
            }
        }
        do {
            try await operation(repository)
            guard repository == activeRepository else { return false }
            _ = await refreshSummary(refreshPullRequestOnHeadChange: false)
            await loadBranches()
            await refreshPullRequest(forceFresh: true)
            postRepositoryChange(repository)
            return true
        } catch {
            guard repository == activeRepository else { return false }
            ToastState.shared.show(title: failureTitle, body: error.localizedDescription)
            return false
        }
    }

    func refreshPullRequest(forceFresh: Bool) async {
        pullRequestRevision += 1
        let revision = pullRequestRevision
        guard let repository = activeRepository,
              let summary,
              !summary.isDetached,
              let headOID = summary.headOID,
              headOID != "(initial)"
        else {
            pullRequestState = .noPullRequest
            pullRequestIdentity = nil
            isRefreshingPullRequest = false
            return
        }
        let identity = PullRequestIdentity(
            repositoryKey: repository.key,
            branch: summary.branch,
            headOID: headOID
        )
        pullRequestState = Self.pullRequestStateForRefresh(
            current: pullRequestState,
            resolvedIdentity: pullRequestIdentity,
            requestedIdentity: identity
        )
        isRefreshingPullRequest = true
        defer {
            if revision == pullRequestRevision {
                isRefreshingPullRequest = false
            }
        }
        let result = await repository.service.cachedPullRequestInfo(
            repoPath: repository.path,
            branch: summary.branch,
            headSha: headOID,
            forceFresh: forceFresh
        )
        guard repository == activeRepository,
              revision == pullRequestRevision,
              self.summary?.branch == identity.branch,
              self.summary?.headOID == identity.headOID
        else { return }
        pullRequestIdentity = identity
        switch result {
        case let .found(info):
            pullRequestState = .found(info)
        case .noPR:
            pullRequestState = .noPullRequest
        case .failed:
            pullRequestState = .unavailable
        }
    }

    func mergePullRequest(
        _ info: GitRepositoryService.PRInfo,
        method: GitRepositoryService.PRMergeMethod
    ) async {
        guard let repository = activeRepository,
              !isMutatingBranches,
              !isMutatingChanges,
              pullRequest == info,
              !isPerformingPullRequestAction
        else { return }
        isMergingPullRequest = true
        defer {
            if repository == activeRepository {
                isMergingPullRequest = false
            }
        }
        do {
            try await repository.service.mergePullRequest(
                repoPath: repository.path,
                number: info.number,
                method: method,
                deleteBranch: false
            )
        } catch {
            guard repository == activeRepository else { return }
            ToastState.shared.show(title: "Failed to merge PR #\(info.number)", body: error.localizedDescription)
            return
        }
        guard repository == activeRepository else { return }
        await checkoutBaseBranch(info.baseBranch, on: repository)
        guard repository == activeRepository else { return }
        ToastState.shared.show("Merged PR #\(info.number) into \(info.baseBranch)")
        _ = await refreshSummary(refreshPullRequestOnHeadChange: false)
        await loadBranches()
        await refreshPullRequest(forceFresh: true)
        postRepositoryChange(repository)
    }

    private func checkoutBaseBranch(_ baseBranch: String, on repository: ActiveRepository) async {
        do {
            try await repository.service.switchBranch(repoPath: repository.path, branch: baseBranch)
            guard repository == activeRepository else { return }
            try await repository.service.pull(repoPath: repository.path)
        } catch {
            guard repository == activeRepository else { return }
            ToastState.shared.show(
                title: "Merged, but couldn't switch to \(baseBranch)",
                body: error.localizedDescription
            )
        }
    }

    func closePullRequest(_ info: GitRepositoryService.PRInfo) async {
        guard let repository = activeRepository,
              !isMutatingBranches,
              !isMutatingChanges,
              pullRequest == info,
              !isPerformingPullRequestAction
        else { return }
        isClosingPullRequest = true
        defer {
            if repository == activeRepository {
                isClosingPullRequest = false
            }
        }
        do {
            try await repository.service.closePullRequest(repoPath: repository.path, number: info.number)
            guard repository == activeRepository else { return }
            ToastState.shared.show("Closed PR #\(info.number)")
            await refreshPullRequest(forceFresh: true)
            postRepositoryChange(repository)
        } catch {
            guard repository == activeRepository else { return }
            ToastState.shared.show(title: "Failed to close PR #\(info.number)", body: error.localizedDescription)
        }
    }

    func updatePullRequestBranch(_ info: GitRepositoryService.PRInfo) async {
        guard let repository = activeRepository,
              !isMutatingBranches,
              !isMutatingChanges,
              pullRequest == info,
              !isPerformingPullRequestAction
        else { return }
        isUpdatingPullRequestBranch = true
        defer {
            if repository == activeRepository {
                isUpdatingPullRequestBranch = false
            }
        }
        do {
            try await repository.service.mergeBaseIntoCurrentBranch(
                repoPath: repository.path,
                baseBranch: info.baseBranch
            )
            guard repository == activeRepository else { return }
            ToastState.shared.show("Updated branch from \(info.baseBranch)")
            _ = await refreshSummary(refreshPullRequestOnHeadChange: false)
            await refreshPullRequest(forceFresh: true)
            postRepositoryChange(repository)
        } catch {
            guard repository == activeRepository else { return }
            ToastState.shared.show(title: "Failed to update branch", body: error.localizedDescription)
        }
    }

    func shouldHandle(_ notification: Notification) -> Bool {
        guard let repository = activeRepository else { return false }
        guard notification.userInfo?[Self.notificationOriginKey] as? String != Self.notificationOriginID else {
            return false
        }
        guard let path = notification.userInfo?["repoPath"] as? String else { return false }
        return path == repository.path
    }

    private func reset(for repository: ActiveRepository) {
        activeRepository = repository
        summary = nil
        branches = []
        resetChangesPresentation(hasLoaded: false)
        pullRequestState = .loading
        pullRequestIdentity = nil
        summaryError = nil
        changesError = nil
        isChangesMonitoringEnabled = false
        resetTransientState()
        summaryRevision += 1
        branchesRevision += 1
        changesRevision += 1
        workingTreeRefreshRevision += 1
        pullRequestRevision += 1
        fileRefreshTask?.cancel()
        fileRefreshTask = nil
        fileSystemWatcher = nil
        gitDirectoryFileSystemWatcher = nil
        guard !repository.context.isRemote else { return }
        let repositoryKey = repository.key
        fileSystemWatcher = makeFileSystemWatcher(
            directoryPath: repository.path,
            repositoryKey: repositoryKey
        )
        guard let gitDirectory = GitWorktreesWatcher.resolveWorktreeGitDirectory(forRepoPath: repository.path),
              gitDirectory != repository.dotGitPath
        else { return }
        gitDirectoryFileSystemWatcher = makeFileSystemWatcher(
            directoryPath: gitDirectory,
            repositoryKey: repositoryKey
        )
    }

    private func makeFileSystemWatcher(
        directoryPath: String,
        repositoryKey: String
    ) -> FileSystemWatcher? {
        FileSystemWatcher(directoryPath: directoryPath) { [weak self] _ in
            Task { @MainActor [weak self] in
                self?.scheduleFileRefresh(for: repositoryKey)
            }
        }
    }

    @discardableResult
    private func refreshSummary(refreshPullRequestOnHeadChange: Bool = true) async -> Bool {
        guard let repository = activeRepository else { return false }
        summaryRevision += 1
        let revision = summaryRevision
        isLoadingSummary = true
        summaryError = nil
        defer {
            if revision == summaryRevision {
                isLoadingSummary = false
            }
        }
        do {
            let loaded = try await repository.service.repositorySummary(repoPath: repository.path)
            guard repository == activeRepository, revision == summaryRevision else { return false }
            let previous = summary
            summary = loaded
            if !loaded.isDirty {
                changesRevision += 1
                resetChangesPresentation(hasLoaded: true)
                changesError = nil
                isLoadingChanges = false
            } else if previous?.isDirty != true {
                resetChangesPresentation(hasLoaded: false)
            }
            let pullRequestIdentityChanged = previous.map {
                $0.branch != loaded.branch || $0.headOID != loaded.headOID
            } ?? false
            if pullRequestIdentityChanged {
                pullRequestRevision += 1
                pullRequestState = .loading
                pullRequestIdentity = nil
                isRefreshingPullRequest = false
            }
            if refreshPullRequestOnHeadChange, pullRequestIdentityChanged {
                await refreshPullRequest(forceFresh: false)
            }
            return true
        } catch {
            guard !Task.isCancelled else { return false }
            guard repository == activeRepository, revision == summaryRevision else { return false }
            logger
                .error("Repository status failed for \(repository.path, privacy: .public): \(error.localizedDescription, privacy: .public)")
            pullRequestRevision += 1
            summary = nil
            branches = []
            resetChangesPresentation(hasLoaded: false)
            pullRequestState = .unavailable
            pullRequestIdentity = nil
            isRefreshingPullRequest = false
            summaryError = error.localizedDescription
            changesError = nil
            return false
        }
    }

    private func resetTransientState() {
        isLoadingSummary = false
        isLoadingBranches = false
        isLoadingChanges = false
        isMutatingChanges = false
        isRefreshingPullRequest = false
        isSwitchingBranch = false
        branchBeingDeleted = nil
        isMergingPullRequest = false
        isClosingPullRequest = false
        isUpdatingPullRequestBranch = false
    }

    private func resetChangesPresentation(hasLoaded: Bool) {
        changesSnapshot = .empty
        untrackedLineStats = [:]
        untrackedLineStatsSummary = RepositoryChangesSnapshot.empty.totalLineStats
        hasLoadedChanges = hasLoaded
        loadedUntrackedLineStats = []
        loadingUntrackedLineStats = []
    }

    private var isPerformingPullRequestAction: Bool {
        isMergingPullRequest || isClosingPullRequest || isUpdatingPullRequestBranch
    }

    nonisolated static func pullRequestStateForRefresh(
        current: PullRequestFetchState,
        resolvedIdentity: PullRequestIdentity?,
        requestedIdentity: PullRequestIdentity
    ) -> PullRequestFetchState {
        resolvedIdentity == requestedIdentity ? current : .loading
    }

    private func scheduleFileRefresh(for repositoryKey: String) {
        guard repositoryKey == activeRepository?.key else { return }
        fileRefreshTask?.cancel()
        fileRefreshTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: 800_000_000)
            guard !Task.isCancelled, let self, repositoryKey == activeRepository?.key else { return }
            guard await refreshSummary() else { return }
            if isChangesMonitoringEnabled {
                await loadChanges()
            }
        }
    }

    private func mutateChanges(
        failureTitle: String,
        operation: (ActiveRepository) async throws -> Void
    ) async {
        guard let repository = activeRepository,
              !isMutatingChanges,
              !isMutatingBranches,
              !isPerformingPullRequestAction
        else { return }
        isMutatingChanges = true
        defer {
            if repository == activeRepository {
                isMutatingChanges = false
            }
        }
        do {
            try await operation(repository)
            guard repository == activeRepository else { return }
            _ = await refreshSummary(refreshPullRequestOnHeadChange: false)
            await loadChanges()
            postRepositoryChange(repository)
        } catch {
            guard repository == activeRepository else { return }
            ToastState.shared.show(title: failureTitle, body: error.localizedDescription)
        }
    }

    private func postRepositoryChange(_ repository: ActiveRepository) {
        NotificationCenter.default.post(
            name: .vcsDidRefresh,
            object: nil,
            userInfo: [
                "repoPath": repository.path,
                Self.notificationOriginKey: Self.notificationOriginID,
            ]
        )
    }
}

private extension TabFocusedRepositoryState {
    struct ActiveRepository: Equatable {
        let path: String
        let context: WorkspaceContext

        var key: String { "\(context.cacheKeyPrefix)|\(path)" }

        var service: GitRepositoryService {
            GitRepositoryService(context: context)
        }

        var dotGitPath: String {
            URL(fileURLWithPath: path)
                .appendingPathComponent(".git")
                .standardizedFileURL
                .path
        }
    }
}
