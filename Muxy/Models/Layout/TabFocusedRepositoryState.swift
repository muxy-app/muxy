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

    static let notificationOriginKey = "muxy.repositoryToolbar.origin"
    static let notificationOriginID = "tabFocusedRepositoryToolbar"

    private(set) var summary: GitRepositorySummary?
    private(set) var branches: [String] = []
    private(set) var pullRequestState: PullRequestFetchState = .loading
    private(set) var isLoadingSummary = false
    private(set) var isLoadingBranches = false
    private(set) var isRefreshingPullRequest = false
    private(set) var isSwitchingBranch = false
    private(set) var isMergingPullRequest = false
    private(set) var isClosingPullRequest = false
    private(set) var isUpdatingPullRequestBranch = false
    private(set) var summaryError: String?

    @ObservationIgnored private var activeRepository: ActiveRepository?
    @ObservationIgnored private var fileSystemWatcher: FileSystemWatcher?
    @ObservationIgnored private var gitDirectoryFileSystemWatcher: FileSystemWatcher?
    @ObservationIgnored private var fileRefreshTask: Task<Void, Never>?
    @ObservationIgnored private var summaryRevision = 0
    @ObservationIgnored private var branchesRevision = 0
    @ObservationIgnored private var pullRequestRevision = 0

    var pullRequest: GitRepositoryService.PRInfo? {
        guard case let .found(info) = pullRequestState else { return nil }
        return info
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
        pullRequestRevision += 1
        summary = nil
        branches = []
        pullRequestState = .loading
        summaryError = nil
        resetTransientState()
    }

    func refreshRepositoryDetails() async {
        guard await refreshSummary() else { return }
        await loadBranches()
    }

    func retryRepository() async {
        guard await refreshSummary(refreshPullRequestOnHeadChange: false) else { return }
        await refreshPullRequest(forceFresh: true)
    }

    func refreshAfterAppActivation() async {
        guard await refreshSummary(refreshPullRequestOnHeadChange: false) else { return }
        await refreshPullRequest(forceFresh: false)
    }

    func refreshFromExternalChange() async {
        guard await refreshSummary(refreshPullRequestOnHeadChange: false) else { return }
        await refreshPullRequest(forceFresh: true)
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
        guard let repository = activeRepository,
              branch != summary?.branch,
              !isSwitchingBranch,
              !isPerformingPullRequestAction
        else { return }
        isSwitchingBranch = true
        defer {
            if repository == activeRepository {
                isSwitchingBranch = false
            }
        }
        do {
            try await repository.service.switchBranch(repoPath: repository.path, branch: branch)
            guard repository == activeRepository else { return }
            _ = await refreshSummary(refreshPullRequestOnHeadChange: false)
            await loadBranches()
            await refreshPullRequest(forceFresh: true)
            postRepositoryChange(repository)
        } catch {
            guard repository == activeRepository else { return }
            ToastState.shared.show(title: "Failed to switch branch", body: error.localizedDescription)
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
            isRefreshingPullRequest = false
            return
        }
        if pullRequest == nil {
            pullRequestState = .loading
        }
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
        guard repository == activeRepository, revision == pullRequestRevision else { return }
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
              !isSwitchingBranch,
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
            guard repository == activeRepository else { return }
            ToastState.shared.show("Merged PR #\(info.number)")
            _ = await refreshSummary(refreshPullRequestOnHeadChange: false)
            await refreshPullRequest(forceFresh: true)
            postRepositoryChange(repository)
        } catch {
            guard repository == activeRepository else { return }
            ToastState.shared.show(title: "Failed to merge PR #\(info.number)", body: error.localizedDescription)
        }
    }

    func closePullRequest(_ info: GitRepositoryService.PRInfo) async {
        guard let repository = activeRepository,
              !isSwitchingBranch,
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
              !isSwitchingBranch,
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
        pullRequestState = .loading
        summaryError = nil
        resetTransientState()
        summaryRevision += 1
        branchesRevision += 1
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
            if refreshPullRequestOnHeadChange,
               let previous,
               previous.branch != loaded.branch || previous.headOID != loaded.headOID
            {
                await refreshPullRequest(forceFresh: false)
            }
            return true
        } catch {
            guard repository == activeRepository, revision == summaryRevision else { return false }
            logger
                .error("Repository status failed for \(repository.path, privacy: .public): \(error.localizedDescription, privacy: .public)")
            pullRequestRevision += 1
            summary = nil
            branches = []
            pullRequestState = .unavailable
            isRefreshingPullRequest = false
            summaryError = error.localizedDescription
            return false
        }
    }

    private func resetTransientState() {
        isLoadingSummary = false
        isLoadingBranches = false
        isRefreshingPullRequest = false
        isSwitchingBranch = false
        isMergingPullRequest = false
        isClosingPullRequest = false
        isUpdatingPullRequestBranch = false
    }

    private var isPerformingPullRequestAction: Bool {
        isMergingPullRequest || isClosingPullRequest || isUpdatingPullRequestBranch
    }

    private func scheduleFileRefresh(for repositoryKey: String) {
        guard repositoryKey == activeRepository?.key else { return }
        fileRefreshTask?.cancel()
        fileRefreshTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: 800_000_000)
            guard !Task.isCancelled, let self, repositoryKey == activeRepository?.key else { return }
            _ = await refreshSummary()
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
