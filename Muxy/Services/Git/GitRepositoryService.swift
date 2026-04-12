import Foundation

struct GitRepositoryService {
    typealias PatchAndCompareResult = GitDiffService.PatchAndCompareResult
    typealias DiffHints = GitDiffService.DiffHints

    private let branchService = GitBranchService()
    private let prService = GitPullRequestService()
    private let diffService = GitDiffService()
    private let stagingService = GitStagingService()
    private let commitService = GitCommitService()

    func currentBranch(repoPath: String) async throws -> String {
        try await branchService.currentBranch(repoPath: repoPath)
    }

    func headSha(repoPath: String) async -> String? {
        await branchService.headSha(repoPath: repoPath)
    }

    func aheadBehind(repoPath: String, branch: String) async -> GitAheadBehind {
        await branchService.aheadBehind(repoPath: repoPath, branch: branch)
    }

    func hasRemoteBranch(repoPath: String, branch: String) async -> Bool {
        await branchService.hasRemoteBranch(repoPath: repoPath, branch: branch)
    }

    func listRemoteBranches(repoPath: String) async throws -> [String] {
        try await branchService.listRemoteBranches(repoPath: repoPath)
    }

    func defaultBranch(repoPath: String) async -> String? {
        await branchService.defaultBranch(repoPath: repoPath)
    }

    func listBranches(repoPath: String) async throws -> [String] {
        try await branchService.listBranches(repoPath: repoPath)
    }

    func switchBranch(repoPath: String, branch: String) async throws {
        try await branchService.switchBranch(repoPath: repoPath, branch: branch)
    }

    func createAndSwitchBranch(repoPath: String, name: String) async throws {
        try await branchService.createAndSwitchBranch(repoPath: repoPath, name: name)
    }

    func createBranch(repoPath: String, name: String, startPoint: String) async throws {
        try await branchService.createBranch(repoPath: repoPath, name: name, startPoint: startPoint)
    }

    func isGhInstalled() async -> Bool {
        await prService.isGhInstalled()
    }

    func cachedPullRequestInfo(
        repoPath: String,
        branch: String,
        headSha: String,
        forceFresh: Bool
    ) async -> PRInfo? {
        await prService.cachedPullRequestInfo(repoPath: repoPath, branch: branch, headSha: headSha, forceFresh: forceFresh)
    }

    func pullRequestInfo(repoPath: String, branch: String) async -> PRInfo? {
        await prService.pullRequestInfo(repoPath: repoPath, branch: branch)
    }

    func createPullRequest(
        repoPath: String,
        branch: String,
        baseBranch: String,
        title: String,
        body: String,
        draft: Bool = false
    ) async throws -> PRInfo {
        try await prService.createPullRequest(
            repoPath: repoPath,
            branch: branch,
            baseBranch: baseBranch,
            title: title,
            body: body,
            draft: draft
        )
    }

    func mergePullRequest(
        repoPath: String,
        number: Int,
        method: PRMergeMethod = .merge,
        deleteBranch: Bool = true
    ) async throws {
        try await prService.mergePullRequest(repoPath: repoPath, number: number, method: method, deleteBranch: deleteBranch)
    }

    func closePullRequest(repoPath: String, number: Int) async throws {
        try await prService.closePullRequest(repoPath: repoPath, number: number)
    }

    func deleteRemoteBranch(repoPath: String, branch: String, remote: String = "origin") async throws {
        try await prService.deleteRemoteBranch(repoPath: repoPath, branch: branch, remote: remote)
    }

    func changedFiles(repoPath: String) async throws -> [GitStatusFile] {
        try await diffService.changedFiles(repoPath: repoPath)
    }

    func patchAndCompare(
        repoPath: String,
        filePath: String,
        lineLimit: Int?,
        hints: DiffHints = .unknown
    ) async throws -> PatchAndCompareResult {
        try await diffService.patchAndCompare(repoPath: repoPath, filePath: filePath, lineLimit: lineLimit, hints: hints)
    }

    func stageFiles(repoPath: String, paths: [String]) async throws {
        try await stagingService.stageFiles(repoPath: repoPath, paths: paths)
    }

    func stageAll(repoPath: String) async throws {
        try await stagingService.stageAll(repoPath: repoPath)
    }

    func unstageFiles(repoPath: String, paths: [String]) async throws {
        try await stagingService.unstageFiles(repoPath: repoPath, paths: paths)
    }

    func unstageAll(repoPath: String) async throws {
        try await stagingService.unstageAll(repoPath: repoPath)
    }

    func discardFiles(repoPath: String, paths: [String], untrackedPaths: [String]) async throws {
        try await stagingService.discardFiles(repoPath: repoPath, paths: paths, untrackedPaths: untrackedPaths)
    }

    func discardAll(repoPath: String) async throws {
        try await stagingService.discardAll(repoPath: repoPath)
    }

    func commit(repoPath: String, message: String) async throws -> String {
        try await commitService.commit(repoPath: repoPath, message: message)
    }

    func push(repoPath: String) async throws {
        try await commitService.push(repoPath: repoPath)
    }

    func pushSetUpstream(repoPath: String, branch: String) async throws {
        try await commitService.pushSetUpstream(repoPath: repoPath, branch: branch)
    }

    func pull(repoPath: String) async throws {
        try await commitService.pull(repoPath: repoPath)
    }

    func commitLog(repoPath: String, maxCount: Int = 100, skip: Int = 0) async throws -> [GitCommit] {
        try await commitService.commitLog(repoPath: repoPath, maxCount: maxCount, skip: skip)
    }

    func cherryPick(repoPath: String, hash: String) async throws {
        try await commitService.cherryPick(repoPath: repoPath, hash: hash)
    }

    func createTag(repoPath: String, name: String, hash: String) async throws {
        try await commitService.createTag(repoPath: repoPath, name: name, hash: hash)
    }

    func checkoutDetached(repoPath: String, hash: String) async throws {
        try await commitService.checkoutDetached(repoPath: repoPath, hash: hash)
    }
}
