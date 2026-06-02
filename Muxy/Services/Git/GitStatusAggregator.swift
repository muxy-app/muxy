import Foundation

struct GitStatusSnapshot {
    let branch: String
    let aheadBehind: GitRepositoryService.AheadBehind
    let defaultBranch: String?
    let branches: [String]
    let files: [GitStatusFile]
    let pullRequest: GitRepositoryService.PRInfo?

    var stagedFiles: [GitStatusFile] { files.filter(\.isStaged) }
    var unstagedFiles: [GitStatusFile] { files.filter(\.isUnstaged) }
}

enum GitStatusAggregator {
    static func snapshot(
        repoPath: String,
        includePullRequest: Bool = true,
        forceFreshPullRequest: Bool = false,
        git: GitRepositoryService = GitRepositoryService()
    ) async throws -> GitStatusSnapshot {
        let branch = try await git.currentBranch(repoPath: repoPath)

        async let filesTask = git.changedFiles(repoPath: repoPath)
        async let branchesTask = try? git.listBranches(repoPath: repoPath)
        async let aheadBehindTask = git.aheadBehind(repoPath: repoPath, branch: branch)
        async let defaultBranchTask = git.defaultBranch(repoPath: repoPath)

        let files = try await filesTask
        let branches = await branchesTask ?? []
        let aheadBehind = await aheadBehindTask
        let defaultBranch = await defaultBranchTask

        let pullRequest = includePullRequest
            ? await pullRequestInfo(repoPath: repoPath, branch: branch, forceFresh: forceFreshPullRequest, git: git)
            : nil

        return GitStatusSnapshot(
            branch: branch,
            aheadBehind: aheadBehind,
            defaultBranch: defaultBranch,
            branches: branches,
            files: files,
            pullRequest: pullRequest
        )
    }

    private static func pullRequestInfo(
        repoPath: String,
        branch: String,
        forceFresh: Bool,
        git: GitRepositoryService
    ) async -> GitRepositoryService.PRInfo? {
        guard let headSha = await git.headSha(repoPath: repoPath) else { return nil }
        let result = await git.cachedPullRequestInfo(
            repoPath: repoPath,
            branch: branch,
            headSha: headSha,
            forceFresh: forceFresh
        )
        guard case let .found(info) = result else { return nil }
        return info
    }
}
