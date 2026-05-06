import Foundation

protocol GitRepositoryServicing {
    // MARK: Branch state

    func currentBranch(repoPath: String) async throws -> String
    func headSha(repoPath: String) async -> String?
    func aheadBehind(repoPath: String, branch: String) async -> GitRepositoryService.AheadBehind
    func defaultBranch(repoPath: String) async -> String?
    func listBranches(repoPath: String) async throws -> [String]
    func switchBranch(repoPath: String, branch: String) async throws
    func createAndSwitchBranch(repoPath: String, name: String) async throws

    // MARK: Remotes

    func hasRemoteBranch(repoPath: String, branch: String) async -> Bool
    func listRemoteBranches(repoPath: String) async throws -> [String]
    func remoteWebURL(repoPath: String, remote: String) async -> URL?
    func deleteRemoteBranch(repoPath: String, branch: String, remote: String) async throws

    // MARK: Working tree

    func changedFiles(repoPath: String) async throws -> [GitStatusFile]
    func patchAndCompare(
        repoPath: String,
        filePath: String,
        lineLimit: Int?,
        hints: GitRepositoryService.DiffHints
    ) async throws -> GitRepositoryService.PatchAndCompareResult

    // MARK: Staging

    func stageFiles(repoPath: String, paths: [String]) async throws
    func stageAll(repoPath: String) async throws
    func unstageFiles(repoPath: String, paths: [String]) async throws
    func unstageAll(repoPath: String) async throws
    func discardFiles(repoPath: String, paths: [String], untrackedPaths: [String]) async throws
    func discardAll(repoPath: String) async throws

    // MARK: Commits

    func commit(repoPath: String, message: String) async throws -> String
    func push(repoPath: String) async throws
    func pushSetUpstream(repoPath: String, branch: String) async throws
    func pull(repoPath: String) async throws
    func commitLog(repoPath: String, maxCount: Int, skip: Int) async throws -> [GitCommit]
    func cherryPick(repoPath: String, hash: String) async throws
    func revert(repoPath: String, hash: String) async throws
    func createBranch(repoPath: String, name: String, startPoint: String) async throws
    func createTag(repoPath: String, name: String, hash: String) async throws
    func checkoutDetached(repoPath: String, hash: String) async throws

    // MARK: Pull requests

    func isGhInstalled() async -> Bool
    func cachedPullRequestInfo(
        repoPath: String,
        branch: String,
        headSha: String,
        forceFresh: Bool
    ) async -> GitRepositoryService.PRInfo?
    func pullRequestInfo(
        repoPath: String,
        branch: String,
        headSha: String?
    ) async -> GitRepositoryService.PRInfo?
    func listPullRequests(
        repoPath: String,
        filter: GitRepositoryService.PRListFilter,
        limit: Int
    ) async throws -> [GitRepositoryService.PRListItem]
    func checkoutPullRequest(repoPath: String, number: Int) async throws
    // swiftlint:disable:next function_parameter_count
    func createPullRequest(
        repoPath: String,
        branch: String,
        baseBranch: String,
        title: String,
        body: String,
        draft: Bool
    ) async throws -> GitRepositoryService.PRInfo
    func mergePullRequest(
        repoPath: String,
        number: Int,
        method: GitRepositoryService.PRMergeMethod,
        deleteBranch: Bool
    ) async throws
    func closePullRequest(repoPath: String, number: Int) async throws
}

extension GitRepositoryServicing {
    func remoteWebURL(repoPath: String) async -> URL? {
        await remoteWebURL(repoPath: repoPath, remote: "origin")
    }

    func deleteRemoteBranch(repoPath: String, branch: String) async throws {
        try await deleteRemoteBranch(repoPath: repoPath, branch: branch, remote: "origin")
    }

    func patchAndCompare(
        repoPath: String,
        filePath: String,
        lineLimit: Int?
    ) async throws -> GitRepositoryService.PatchAndCompareResult {
        try await patchAndCompare(
            repoPath: repoPath,
            filePath: filePath,
            lineLimit: lineLimit,
            hints: .unknown
        )
    }

    func commitLog(repoPath: String, maxCount: Int = 100, skip: Int = 0) async throws -> [GitCommit] {
        try await commitLog(repoPath: repoPath, maxCount: maxCount, skip: skip)
    }

    func pullRequestInfo(repoPath: String, branch: String) async -> GitRepositoryService.PRInfo? {
        await pullRequestInfo(repoPath: repoPath, branch: branch, headSha: nil)
    }

    func listPullRequests(
        repoPath: String,
        filter: GitRepositoryService.PRListFilter = .open,
        limit: Int = 100
    ) async throws -> [GitRepositoryService.PRListItem] {
        try await listPullRequests(repoPath: repoPath, filter: filter, limit: limit)
    }

    func createPullRequest(
        repoPath: String,
        branch: String,
        baseBranch: String,
        title: String,
        body: String
    ) async throws -> GitRepositoryService.PRInfo {
        try await createPullRequest(
            repoPath: repoPath,
            branch: branch,
            baseBranch: baseBranch,
            title: title,
            body: body,
            draft: false
        )
    }

    func mergePullRequest(
        repoPath: String,
        number: Int,
        method: GitRepositoryService.PRMergeMethod = .merge
    ) async throws {
        try await mergePullRequest(
            repoPath: repoPath,
            number: number,
            method: method,
            deleteBranch: true
        )
    }
}

extension GitRepositoryService: GitRepositoryServicing {}
