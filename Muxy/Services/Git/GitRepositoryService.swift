import Foundation

struct GitRepositoryService {
    struct PatchAndCompareResult {
        let rows: [DiffDisplayRow]
        let truncated: Bool
        let additions: Int
        let deletions: Int
    }

    struct DiffRange: Equatable {
        let baseRef: String
        let headRef: String

        var spec: String { "\(baseRef)...\(headRef)" }
    }

    enum GitError: LocalizedError {
        case notGitRepository
        case noUpstreamBranch
        case commandFailed(String)

        var errorDescription: String? {
            switch self {
            case .notGitRepository:
                "This folder is not a Git repository."
            case .noUpstreamBranch:
                "The current branch has no upstream branch on the remote."
            case let .commandFailed(message):
                message
            }
        }
    }

    struct PRInfo: Equatable {
        let url: String
        let number: Int
        let state: PRState
        let isDraft: Bool
        let baseBranch: String
        let mergeable: Bool?
        let mergeStateStatus: PRMergeStateStatus
        let checks: PRChecks
        let isCrossRepository: Bool
    }

    struct PRCheckoutInfo: Equatable {
        let number: Int
        let headBranch: String
        let headRepositoryNameWithOwner: String
    }

    struct PRListItem: Equatable, Identifiable {
        let number: Int
        let title: String
        let author: String
        let headBranch: String
        let headRefOid: String
        let baseBranch: String
        let state: PRState
        let isDraft: Bool
        let url: String
        let updatedAt: Date?
        let checks: PRChecks
        let mergeable: Bool?
        let mergeStateStatus: PRMergeStateStatus

        var id: Int { number }
    }

    enum PRState: String {
        case open = "OPEN"
        case closed = "CLOSED"
        case merged = "MERGED"
    }

    enum PRMergeStateStatus: String {
        case clean = "CLEAN"
        case hasHooks = "HAS_HOOKS"
        case unstable = "UNSTABLE"
        case behind = "BEHIND"
        case blocked = "BLOCKED"
        case dirty = "DIRTY"
        case draft = "DRAFT"
        case unknown = "UNKNOWN"
    }

    struct PRChecks: Equatable {
        let status: PRChecksStatus
        let passing: Int
        let failing: Int
        let pending: Int
        let total: Int
    }

    enum PRChecksStatus: Equatable {
        case none
        case pending
        case success
        case failure
    }

    struct AheadBehind: Equatable {
        let ahead: Int
        let behind: Int
        let hasUpstream: Bool
    }

    enum PRCreateError: LocalizedError {
        case ghNotInstalled
        case commandFailed(String)

        var errorDescription: String? {
            switch self {
            case .ghNotInstalled:
                "GitHub CLI (gh) is not installed. Install it with `brew install gh`."
            case let .commandFailed(message):
                message
            }
        }
    }

    enum PRMergeMethod: String, CaseIterable, Identifiable {
        case squash
        case merge
        case rebase

        var id: String { rawValue }

        var ghFlag: String {
            switch self {
            case .merge: "--merge"
            case .squash: "--squash"
            case .rebase: "--rebase"
            }
        }

        var shortLabel: String {
            switch self {
            case .merge: "Merge"
            case .squash: "Squash"
            case .rebase: "Rebase"
            }
        }

        var label: String {
            switch self {
            case .merge: "Merge Commit"
            case .squash: "Squash and Merge"
            case .rebase: "Rebase and Merge"
            }
        }
    }

    struct DiffHints {
        let hasStaged: Bool
        let hasUnstaged: Bool
        let isUntrackedOrNew: Bool

        static let unknown = DiffHints(hasStaged: true, hasUnstaged: true, isUntrackedOrNew: false)
    }

    func currentBranch(repoPath: String) async throws -> String {
        let result = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["rev-parse", "--abbrev-ref", "HEAD"]
        )
        guard result.status == 0 else {
            throw GitError.commandFailed("Failed to get current branch.")
        }
        return result.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    func headSha(repoPath: String) async -> String? {
        let result = try? await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["rev-parse", "HEAD"]
        )
        guard let result, result.status == 0 else { return nil }
        let trimmed = result.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }

    func isGhInstalled() async -> Bool {
        if let cached = GitMetadataCache.shared.cachedGhInstalled() {
            return cached
        }
        let installed = GitProcessRunner.resolveExecutable("gh") != nil
        GitMetadataCache.shared.storeGhInstalled(installed)
        return installed
    }

    enum PRFetchResult: Equatable {
        case found(PRInfo)
        case noPR
        case failed
    }

    func cachedPullRequestInfo(
        repoPath: String,
        branch: String,
        headSha: String,
        forceFresh: Bool
    ) async -> PRFetchResult {
        if !forceFresh, let cached = GitMetadataCache.shared.cachedPRInfo(
            repoPath: repoPath,
            branch: branch,
            headSha: headSha
        ) {
            return cached.map { .found($0) } ?? .noPR
        }
        let result = await pullRequestInfoResult(repoPath: repoPath, branch: branch, headSha: headSha)
        switch result {
        case let .found(info):
            GitMetadataCache.shared.storePRInfo(info, repoPath: repoPath, branch: branch, headSha: headSha)
        case .noPR:
            GitMetadataCache.shared.storePRInfo(nil, repoPath: repoPath, branch: branch, headSha: headSha)
        case .failed:
            break
        }
        return result
    }

    static let prInfoJSONFields =
        "url,number,state,isDraft,baseRefName,mergeable,mergeStateStatus,statusCheckRollup,isCrossRepository"
    static let prInfoJSONFieldsWithHeadRefOid = prInfoJSONFields + ",headRefOid,headRefName"
    static let prCheckoutJSONFields = "number,headRefName,headRepository"

    func pullRequestInfo(repoPath: String, branch: String, headSha: String? = nil) async -> PRInfo? {
        if case let .found(info) = await pullRequestInfoResult(
            repoPath: repoPath, branch: branch, headSha: headSha
        ) {
            return info
        }
        return nil
    }

    func pullRequestInfoResult(
        repoPath: String,
        branch: String,
        headSha: String? = nil
    ) async -> PRFetchResult {
        guard let ghPath = GitProcessRunner.resolveExecutable("gh") else { return .failed }

        if let number = await configuredPullRequestNumber(repoPath: repoPath, branch: branch) {
            let configuredResult = await ghPRView(
                ghPath: ghPath,
                repoPath: repoPath,
                argument: String(number),
                jsonFields: Self.prInfoJSONFields
            )
            if case let .found(info) = configuredResult { return .found(info) }
        }

        let viewResult = await ghPRView(ghPath: ghPath, repoPath: repoPath, jsonFields: Self.prInfoJSONFields)
        if case let .found(info) = viewResult { return .found(info) }

        let viewByBranch = await ghPRView(
            ghPath: ghPath, repoPath: repoPath, argument: branch, jsonFields: Self.prInfoJSONFields
        )
        if case let .found(info) = viewByBranch { return .found(info) }

        let resolvedSha: String? = if let headSha { headSha } else { await self.headSha(repoPath: repoPath) }
        if let resolvedSha {
            let byShaResult = await pullRequestInfoByHeadSha(
                ghPath: ghPath, repoPath: repoPath, branch: branch, headSha: resolvedSha
            )
            if case let .found(info) = byShaResult { return .found(info) }
            if byShaResult == .failed { return .failed }
        }

        if viewResult == .failed || viewByBranch == .failed { return .failed }
        return .noPR
    }

    private func pullRequestInfoByHeadSha(
        ghPath: String,
        repoPath: String,
        branch: String,
        headSha: String
    ) async -> PRFetchResult {
        let arguments = [
            "pr", "list",
            "--state", "all",
            "--head", branch,
            "--limit", "100",
            "--json", Self.prInfoJSONFieldsWithHeadRefOid,
        ]
        let result = try? await GitProcessRunner.runCommand(
            executable: ghPath,
            arguments: arguments,
            workingDirectory: repoPath
        )
        guard let result else { return .failed }
        guard result.status == 0 else {
            return ghErrorIndicatesNoPR(stderr: result.stderr) ? .noPR : .failed
        }
        if let info = GitPRParser.parsePRInfoMatchingHeadSha(
            result.stdout, headSha: headSha, branch: branch
        ) {
            return .found(info)
        }
        return .noPR
    }

    private func ghPRView(
        ghPath: String,
        repoPath: String,
        argument: String? = nil,
        jsonFields: String
    ) async -> PRFetchResult {
        var arguments = ["pr", "view"]
        if let argument {
            arguments.append(argument)
        }
        arguments += ["--json", jsonFields]

        let result = try? await GitProcessRunner.runCommand(
            executable: ghPath,
            arguments: arguments,
            workingDirectory: repoPath
        )
        guard let result else { return .failed }
        guard result.status == 0 else {
            return ghErrorIndicatesNoPR(stderr: result.stderr) ? .noPR : .failed
        }
        if let info = GitPRParser.parsePRInfo(result.stdout) {
            return .found(info)
        }
        return .noPR
    }

    private func configuredPullRequestNumber(repoPath: String, branch: String) async -> Int? {
        guard !branch.isEmpty,
              branch.unicodeScalars.allSatisfy({ Self.allowedBranchCharacters.contains($0) })
        else { return nil }
        let result = try? await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["config", "--get", "branch.\(branch).muxy-pr-number"]
        )
        guard let result, result.status == 0 else { return nil }
        return Int(result.stdout.trimmingCharacters(in: .whitespacesAndNewlines))
    }

    enum PRCommentSide: String {
        case left = "LEFT"
        case right = "RIGHT"
    }

    enum PostCommentResult {
        case success
        case failure(String)
    }

    struct GitHubUser: Equatable {
        let login: String
        let avatarURL: URL?
    }

    func currentGitHubUser(repoPath: String) async -> GitHubUser? {
        guard let ghPath = GitProcessRunner.resolveExecutable("gh") else { return nil }
        let result = try? await GitProcessRunner.runCommand(
            executable: ghPath,
            arguments: ["api", "user", "--jq", "{login: .login, avatar_url: .avatar_url}"],
            workingDirectory: repoPath
        )
        guard let result, result.status == 0,
              let data = result.stdout.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let login = object["login"] as? String
        else { return nil }
        let avatarURL = (object["avatar_url"] as? String).flatMap(URL.init(string:))
        return GitHubUser(login: login, avatarURL: avatarURL)
    }

    struct PRCommentRequest {
        let repoPath: String
        let number: Int
        let commit: String
        let path: String
        let line: Int
        let side: PRCommentSide
        let body: String
    }

    func postPullRequestReviewComment(_ request: PRCommentRequest) async -> PostCommentResult {
        guard let ghPath = GitProcessRunner.resolveExecutable("gh") else {
            return .failure("GitHub CLI (gh) is not installed.")
        }
        guard let commitSha = await resolveCommitSha(repoPath: request.repoPath, ref: request.commit) else {
            return .failure("Could not resolve the pull request head commit.")
        }
        let arguments = [
            "api",
            "--method", "POST",
            "repos/{owner}/{repo}/pulls/\(request.number)/comments",
            "-f", "body=\(request.body)",
            "-f", "commit_id=\(commitSha)",
            "-f", "path=\(request.path)",
            "-F", "line=\(request.line)",
            "-f", "side=\(request.side.rawValue)",
        ]
        let result = try? await GitProcessRunner.runCommand(
            executable: ghPath,
            arguments: arguments,
            workingDirectory: request.repoPath
        )
        guard let result else { return .failure("Failed to run gh.") }
        guard result.status == 0 else {
            let message = result.stderr.trimmingCharacters(in: .whitespacesAndNewlines)
            return .failure(message.isEmpty ? "Failed to post comment." : message)
        }
        return .success
    }

    private func resolveCommitSha(repoPath: String, ref: String) async -> String? {
        let result = try? await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["rev-parse", ref])
        guard let result, result.status == 0 else { return nil }
        let sha = result.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
        return sha.isEmpty ? nil : sha
    }

    private func ghErrorIndicatesNoPR(stderr: String) -> Bool {
        let lowered = stderr.lowercased()
        return lowered.contains("no pull requests found")
            || lowered.contains("no pull request found")
            || lowered.contains("could not resolve")
            || lowered.contains("no commits between")
    }

    enum PRListFilter: String {
        case open
        case closed
        case merged
        case all
    }

    func listPullRequests(
        repoPath: String,
        filter: PRListFilter = .open,
        limit: Int = 100
    ) async throws -> [PRListItem] {
        guard let ghPath = GitProcessRunner.resolveExecutable("gh") else {
            throw PRCreateError.ghNotInstalled
        }
        let jsonFields = [
            "number", "title", "author",
            "headRefName", "headRefOid", "baseRefName",
            "state", "isDraft", "url", "updatedAt",
            "statusCheckRollup", "mergeable", "mergeStateStatus",
        ].joined(separator: ",")
        let arguments = [
            "pr", "list",
            "--state", filter.rawValue,
            "--limit", String(limit),
            "--json", jsonFields,
        ]
        let result = try await GitProcessRunner.runCommand(
            executable: ghPath,
            arguments: arguments,
            workingDirectory: repoPath
        )
        guard result.status == 0 else {
            let message = result.stderr.isEmpty ? result.stdout : result.stderr
            throw PRCreateError.commandFailed(
                message.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                    ? "Failed to list pull requests."
                    : message.trimmingCharacters(in: .whitespacesAndNewlines)
            )
        }
        return GitPRParser.parsePRList(result.stdout)
    }

    func checkoutPullRequest(repoPath: String, number: Int, headBranch _: String? = nil) async throws {
        guard let ghPath = GitProcessRunner.resolveExecutable("gh") else {
            throw PRCreateError.ghNotInstalled
        }
        let checkout = try await pullRequestCheckoutInfo(ghPath: ghPath, repoPath: repoPath, number: number)
        try await preparePullRequestBranch(repoPath: repoPath, checkout: checkout)
        let result = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["switch", Self.localPullRequestBranchName(for: checkout)]
        )
        try requireSuccess(result, fallbackMessage: "Failed to checkout pull request.")
    }

    func createPullRequestWorktree(repoPath: String, path: String, number: Int) async throws -> String {
        guard let ghPath = GitProcessRunner.resolveExecutable("gh") else {
            throw PRCreateError.ghNotInstalled
        }
        let checkout = try await pullRequestCheckoutInfo(ghPath: ghPath, repoPath: repoPath, number: number)
        try await preparePullRequestBranch(repoPath: repoPath, checkout: checkout)
        let branch = Self.localPullRequestBranchName(for: checkout)
        try await GitWorktreeService.shared.addWorktree(
            repoPath: repoPath,
            path: path,
            branch: branch,
            createBranch: false
        )
        return branch
    }

    private func pullRequestCheckoutInfo(
        ghPath: String,
        repoPath: String,
        number: Int
    ) async throws -> PRCheckoutInfo {
        let result = try await GitProcessRunner.runCommand(
            executable: ghPath,
            arguments: ["pr", "view", String(number), "--json", Self.prCheckoutJSONFields],
            workingDirectory: repoPath
        )
        try requireSuccess(result, fallbackMessage: "Failed to read pull request.")
        guard let checkout = GitPRParser.parsePRCheckoutInfo(result.stdout) else {
            throw PRCreateError.commandFailed("Failed to read pull request checkout metadata.")
        }
        return checkout
    }

    private func preparePullRequestBranch(repoPath: String, checkout: PRCheckoutInfo) async throws {
        let remote = try await ensurePullRequestRemote(repoPath: repoPath, checkout: checkout)
        let branch = Self.localPullRequestBranchName(for: checkout)
        let fetchResult = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["fetch", remote, "refs/heads/\(checkout.headBranch):refs/remotes/\(remote)/\(checkout.headBranch)"]
        )
        try requireSuccess(fetchResult, fallbackMessage: "Failed to fetch pull request branch.")

        let refExists = await localBranchExists(repoPath: repoPath, branch: branch)
        let startPoint = "refs/remotes/\(remote)/\(checkout.headBranch)"
        let branchResult = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: refExists ? ["branch", "--set-upstream-to=\(remote)/\(checkout.headBranch)", branch]
                : ["branch", "--track", branch, startPoint]
        )
        try requireSuccess(branchResult, fallbackMessage: "Failed to prepare pull request branch.")

        let configResult = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["config", "branch.\(branch).muxy-pr-number", String(checkout.number)]
        )
        try requireSuccess(configResult, fallbackMessage: "Failed to store pull request metadata.")
    }

    private func ensurePullRequestRemote(repoPath: String, checkout: PRCheckoutInfo) async throws -> String {
        let remote = Self.pullRequestRemoteName(for: checkout)
        if await remoteExists(repoPath: repoPath, remote: remote) {
            return remote
        }
        let result = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["remote", "add", remote, "https://github.com/\(checkout.headRepositoryNameWithOwner).git"]
        )
        try requireSuccess(result, fallbackMessage: "Failed to add pull request remote.")
        return remote
    }

    private func localBranchExists(repoPath: String, branch: String) async -> Bool {
        let result = try? await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["show-ref", "--verify", "--quiet", "refs/heads/\(branch)"]
        )
        return result?.status == 0
    }

    private func remoteExists(repoPath: String, remote: String) async -> Bool {
        let result = try? await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["remote"])
        guard let result, result.status == 0 else { return false }
        return result.stdout.split(separator: "\n").contains { $0 == remote }
    }

    static func localPullRequestBranchName(for checkout: PRCheckoutInfo) -> String {
        "pr/\(checkout.number)/\(safeRefComponent(checkout.headBranch))"
    }

    static func pullRequestRemoteName(for checkout: PRCheckoutInfo) -> String {
        "pr-\(checkout.number)-\(safeRefComponent(checkout.headRepositoryNameWithOwner).replacingOccurrences(of: "/", with: "-"))"
    }

    private static func safeRefComponent(_ value: String) -> String {
        let segments = value
            .split(separator: "/", omittingEmptySubsequences: true)
            .map { safeRefSegment(String($0)) }
            .filter { !$0.isEmpty }
        return segments.isEmpty ? "head" : segments.joined(separator: "/")
    }

    private static func safeRefSegment(_ value: String) -> String {
        let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "_-"))
        let scalars = value.unicodeScalars.map { allowed.contains($0) ? Character($0) : "-" }
        return String(scalars).split(separator: "-", omittingEmptySubsequences: true).joined(separator: "-")
    }

    private func requireSuccess(_ result: GitProcessResult, fallbackMessage: String) throws {
        guard result.status == 0 else {
            let message = result.stderr.isEmpty ? result.stdout : result.stderr
            let trimmed = message.trimmingCharacters(in: .whitespacesAndNewlines)
            throw PRCreateError.commandFailed(trimmed.isEmpty ? fallbackMessage : trimmed)
        }
    }

    func aheadBehind(repoPath: String, branch: String) async -> AheadBehind {
        async let upstreamTask = GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["rev-parse", "--abbrev-ref", "\(branch)@{upstream}"]
        )
        async let countsTask = GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["rev-list", "--left-right", "--count", "\(branch)...\(branch)@{upstream}"]
        )

        let upstreamResult = try? await upstreamTask
        guard let upstreamResult, upstreamResult.status == 0 else {
            _ = try? await countsTask
            return AheadBehind(ahead: 0, behind: 0, hasUpstream: false)
        }

        guard let countsResult = try? await countsTask, countsResult.status == 0 else {
            return AheadBehind(ahead: 0, behind: 0, hasUpstream: true)
        }
        return GitPRParser.parseAheadBehind(counts: countsResult.stdout, hasUpstream: true)
    }

    func hasRemoteBranch(repoPath: String, branch: String) async -> Bool {
        let result = try? await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["ls-remote", "--heads", "origin", branch]
        )
        guard let result, result.status == 0 else { return false }
        return !result.stdout.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    func listRemoteBranches(repoPath: String) async throws -> [String] {
        let result = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["ls-remote", "--heads", "origin"]
        )
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to list remote branches." : result.stderr)
        }
        let prefix = "refs/heads/"
        return result.stdout
            .split(separator: "\n", omittingEmptySubsequences: true)
            .compactMap { line -> String? in
                let parts = line.split(separator: "\t", maxSplits: 1, omittingEmptySubsequences: true)
                guard parts.count == 2 else { return nil }
                let ref = parts[1].trimmingCharacters(in: .whitespaces)
                guard ref.hasPrefix(prefix) else { return nil }
                return String(ref.dropFirst(prefix.count))
            }
            .sorted { $0.localizedStandardCompare($1) == .orderedAscending }
    }

    func defaultBranch(repoPath: String) async -> String? {
        if let cached = GitMetadataCache.shared.cachedDefaultBranch(repoPath: repoPath) {
            return cached
        }
        let resolved = await resolveDefaultBranch(repoPath: repoPath)
        if resolved != nil {
            GitMetadataCache.shared.storeDefaultBranch(resolved, repoPath: repoPath)
        }
        return resolved
    }

    func remoteWebURL(repoPath: String, remote: String = "origin") async -> URL? {
        if remote == "origin", let cached = GitMetadataCache.shared.cachedRemoteWebURL(repoPath: repoPath) {
            return cached
        }
        let result = try? await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["remote", "get-url", remote]
        )
        guard let result, result.status == 0 else { return nil }
        let raw = result.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
        let url = Self.webURL(fromRemoteURL: raw)
        if remote == "origin" {
            GitMetadataCache.shared.storeRemoteWebURL(url, repoPath: repoPath)
        }
        return url
    }

    func githubRemoteName(repoPath: String) async -> String? {
        guard let ghPath = GitProcessRunner.resolveExecutable("gh") else { return nil }
        let repoResult = try? await GitProcessRunner.runCommand(
            executable: ghPath,
            arguments: ["repo", "view", "--json", "nameWithOwner", "-q", ".nameWithOwner"],
            workingDirectory: repoPath
        )
        guard let repoResult, repoResult.status == 0 else { return nil }
        let nameWithOwner = repoResult.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !nameWithOwner.isEmpty else { return nil }

        let remoteResult = try? await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["remote", "-v"])
        guard let remoteResult, remoteResult.status == 0 else { return nil }
        return Self.githubRemoteName(fromRemoteList: remoteResult.stdout, nameWithOwner: nameWithOwner)
    }

    static func githubRemoteName(fromRemoteList remoteList: String, nameWithOwner: String) -> String? {
        let normalizedTarget = nameWithOwner.lowercased()
        for line in remoteList.split(separator: "\n") {
            let parts = line.split(whereSeparator: \.isWhitespace)
            guard parts.count >= 2 else { continue }
            let remote = String(parts[0])
            guard let path = webURL(fromRemoteURL: String(parts[1]))?.path.dropFirst().lowercased() else { continue }
            if path == normalizedTarget {
                return remote
            }
        }
        return nil
    }

    static func webURL(fromRemoteURL raw: String) -> URL? {
        guard !raw.isEmpty else { return nil }
        var value = raw
        if value.hasSuffix(".git") { value = String(value.dropLast(4)) }

        if value.hasPrefix("http://") || value.hasPrefix("https://") {
            return URL(string: value)
        }

        if value.hasPrefix("ssh://") {
            guard var components = URLComponents(string: value) else { return nil }
            components.scheme = "https"
            components.user = nil
            components.password = nil
            components.port = nil
            return components.url
        }

        if let atIndex = value.firstIndex(of: "@"), let colonIndex = value[atIndex...].firstIndex(of: ":") {
            let host = String(value[value.index(after: atIndex) ..< colonIndex])
            let path = String(value[value.index(after: colonIndex)...])
            return URL(string: "https://\(host)/\(path)")
        }

        return nil
    }

    private func resolveDefaultBranch(repoPath: String) async -> String? {
        let symbolic = try? await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["symbolic-ref", "--short", "refs/remotes/origin/HEAD"]
        )
        if let symbolic, symbolic.status == 0 {
            let value = symbolic.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
            if value.hasPrefix("origin/") {
                return String(value.dropFirst("origin/".count))
            }
            if !value.isEmpty { return value }
        }

        if let ghPath = GitProcessRunner.resolveExecutable("gh") {
            let result = try? await GitProcessRunner.runCommand(
                executable: ghPath,
                arguments: ["repo", "view", "--json", "defaultBranchRef", "-q", ".defaultBranchRef.name"],
                workingDirectory: repoPath
            )
            if let result, result.status == 0 {
                let trimmed = result.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
                if !trimmed.isEmpty { return trimmed }
            }
        }

        return nil
    }

    func createPullRequest(
        repoPath: String,
        branch: String,
        baseBranch: String,
        title: String,
        body: String,
        draft: Bool = false
    ) async throws -> PRInfo {
        guard let ghPath = GitProcessRunner.resolveExecutable("gh") else {
            throw PRCreateError.ghNotInstalled
        }

        var arguments: [String] = [
            "pr", "create",
            "--head", branch,
            "--base", baseBranch,
            "--title", title,
        ]
        arguments.append("--body")
        arguments.append(body)
        if draft {
            arguments.append("--draft")
        }

        let createResult = try await GitProcessRunner.runCommand(
            executable: ghPath,
            arguments: arguments,
            workingDirectory: repoPath
        )
        guard createResult.status == 0 else {
            let message = createResult.stderr.isEmpty ? createResult.stdout : createResult.stderr
            throw PRCreateError.commandFailed(
                message.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                    ? "Failed to create pull request."
                    : message.trimmingCharacters(in: .whitespacesAndNewlines)
            )
        }

        GitMetadataCache.shared.invalidatePRInfo(repoPath: repoPath, branch: branch)

        let createdURL = createResult.stdout
            .split(whereSeparator: { $0.isNewline || $0.isWhitespace })
            .map(String.init)
            .first(where: { $0.hasPrefix("https://") }) ?? ""

        if !createdURL.isEmpty,
           case let .found(info) = await ghPRView(
               ghPath: ghPath,
               repoPath: repoPath,
               argument: createdURL,
               jsonFields: Self.prInfoJSONFields
           )
        {
            return info
        }

        if let info = await pullRequestInfo(repoPath: repoPath, branch: branch) {
            return info
        }

        throw PRCreateError.commandFailed(
            createdURL.isEmpty
                ? "Pull request created but could not be read back."
                : "Pull request created at \(createdURL) but could not be read back."
        )
    }

    func mergePullRequest(
        repoPath: String,
        number: Int,
        method: PRMergeMethod = .merge,
        deleteBranch: Bool = true
    ) async throws {
        guard let ghPath = GitProcessRunner.resolveExecutable("gh") else {
            throw PRCreateError.ghNotInstalled
        }
        var arguments = ["pr", "merge", String(number), method.ghFlag]
        if deleteBranch {
            arguments.append("--delete-branch")
        }
        let result = try await GitProcessRunner.runCommand(
            executable: ghPath,
            arguments: arguments,
            workingDirectory: repoPath
        )
        guard result.status == 0 else {
            let message = result.stderr.isEmpty ? result.stdout : result.stderr
            throw PRCreateError.commandFailed(
                message.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                    ? "Failed to merge pull request."
                    : message.trimmingCharacters(in: .whitespacesAndNewlines)
            )
        }
        GitMetadataCache.shared.invalidatePRInfo(repoPath: repoPath)
    }

    func deleteRemoteBranch(repoPath: String, branch: String, remote: String = "origin") async throws {
        let result = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["push", remote, "--delete", branch]
        )
        guard result.status == 0 else {
            throw GitError.commandFailed(
                result.stderr.isEmpty ? "Failed to delete remote branch \(branch)." : result.stderr
            )
        }
    }

    func closePullRequest(repoPath: String, number: Int) async throws {
        guard let ghPath = GitProcessRunner.resolveExecutable("gh") else {
            throw PRCreateError.ghNotInstalled
        }
        let result = try await GitProcessRunner.runCommand(
            executable: ghPath,
            arguments: ["pr", "close", String(number)],
            workingDirectory: repoPath
        )
        guard result.status == 0 else {
            let message = result.stderr.isEmpty ? result.stdout : result.stderr
            throw PRCreateError.commandFailed(
                message.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                    ? "Failed to close pull request."
                    : message.trimmingCharacters(in: .whitespacesAndNewlines)
            )
        }
        GitMetadataCache.shared.invalidatePRInfo(repoPath: repoPath)
    }

    func changedFiles(repoPath: String) async throws -> [GitStatusFile] {
        let signpostID = GitSignpost.begin("changedFiles")
        defer { GitSignpost.end("changedFiles", signpostID) }

        if !GitMetadataCache.shared.isVerifiedGitRepo(repoPath: repoPath) {
            let verifyResult = try await GitProcessRunner.runGit(
                repoPath: repoPath,
                arguments: ["rev-parse", "--is-inside-work-tree"]
            )
            guard verifyResult.status == 0,
                  verifyResult.stdout.trimmingCharacters(in: .whitespacesAndNewlines) == "true"
            else {
                throw GitError.notGitRepository
            }
            GitMetadataCache.shared.markVerifiedGitRepo(repoPath: repoPath)
        }

        async let statusTask = GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["-c", "core.quotepath=false", "status", "--porcelain=1", "-z", "--untracked-files=all"]
        )
        async let numstatTask = GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["-c", "core.quotepath=false", "diff", "HEAD", "--numstat", "--no-color", "--no-ext-diff"]
        )
        async let stagedNumstatTask = GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["-c", "core.quotepath=false", "diff", "--cached", "--numstat", "--no-color", "--no-ext-diff"]
        )
        async let unstagedNumstatTask = GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["-c", "core.quotepath=false", "diff", "--numstat", "--no-color", "--no-ext-diff"]
        )

        let statusResult = try await statusTask
        guard statusResult.status == 0 else {
            _ = try? await numstatTask
            _ = try? await stagedNumstatTask
            _ = try? await unstagedNumstatTask
            throw GitError.commandFailed(statusResult.stderr.isEmpty ? "Failed to load Git status." : statusResult.stderr)
        }

        let numstatResult = try await numstatTask
        let stagedNumstatResult = try await stagedNumstatTask
        let unstagedNumstatResult = try await unstagedNumstatTask
        let stats = numstatResult.status == 0 ? GitStatusParser.parseNumstat(numstatResult.stdout) : [:]
        let stagedStats = stagedNumstatResult.status == 0 ? GitStatusParser.parseNumstat(stagedNumstatResult.stdout) : [:]
        let unstagedStats = unstagedNumstatResult.status == 0 ? GitStatusParser.parseNumstat(unstagedNumstatResult.stdout) : [:]

        return GitStatusParser.parseStatusPorcelain(statusResult.stdoutData, stats: stats).map { file in
            let staged = stagedStats[file.path]
            let unstaged = unstagedStats[file.path]
            let file = GitStatusFile(
                path: file.path,
                oldPath: file.oldPath,
                xStatus: file.xStatus,
                yStatus: file.yStatus,
                additions: file.additions,
                deletions: file.deletions,
                stagedAdditions: staged?.additions,
                stagedDeletions: staged?.deletions,
                unstagedAdditions: unstaged?.additions,
                unstagedDeletions: unstaged?.deletions,
                isBinary: file.isBinary || staged?.isBinary == true || unstaged?.isBinary == true
            )
            guard file.additions == nil, file.xStatus == "?" || file.xStatus == "A" else { return file }
            let lineCount = Self.countLines(repoPath: repoPath, relativePath: file.path)
            return GitStatusFile(
                path: file.path,
                oldPath: file.oldPath,
                xStatus: file.xStatus,
                yStatus: file.yStatus,
                additions: lineCount,
                deletions: 0,
                stagedAdditions: file.stagedAdditions,
                stagedDeletions: file.stagedDeletions,
                unstagedAdditions: file.unstagedAdditions,
                unstagedDeletions: file.unstagedDeletions,
                isBinary: file.isBinary
            )
        }
    }

    func changedFiles(repoPath: String, range: DiffRange) async throws -> [GitStatusFile] {
        try validateRef(range.baseRef)
        try validateRef(range.headRef)

        async let nameStatusTask = GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["-c", "core.quotepath=false", "diff", "--name-status", "-z", range.spec]
        )
        async let numstatTask = GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["-c", "core.quotepath=false", "diff", "--numstat", "--no-color", "--no-ext-diff", range.spec]
        )

        let nameStatusResult = try await nameStatusTask
        guard nameStatusResult.status == 0 else {
            _ = try? await numstatTask
            throw GitError.commandFailed(nameStatusResult.stderr.isEmpty ? "Failed to load changed files." : nameStatusResult.stderr)
        }

        let statsResult = try await numstatTask
        let stats = statsResult.status == 0 ? GitStatusParser.parseNumstat(statsResult.stdout) : [:]
        return Self.parseNameStatus(nameStatusResult.stdoutData, stats: stats)
    }

    func changedFiles(repoPath: String, commit: String) async throws -> [GitStatusFile] {
        try validateHash(commit)
        return try await changedFiles(repoPath: repoPath, range: DiffRange(baseRef: "\(commit)^", headRef: commit))
    }

    private static func countLines(repoPath: String, relativePath: String) -> Int? {
        let fullPath = (repoPath as NSString).appendingPathComponent(relativePath)
        guard let data = FileManager.default.contents(atPath: fullPath),
              let content = String(data: data, encoding: .utf8)
        else {
            return nil
        }
        return content.isEmpty ? 0 : content.split(omittingEmptySubsequences: false, whereSeparator: \.isNewline).count
    }

    func patchAndCompare(
        repoPath: String,
        filePath: String,
        lineLimit: Int?,
        hints: DiffHints = .unknown
    ) async throws -> PatchAndCompareResult {
        let signpostID = GitSignpost.begin("patchAndCompare", filePath)
        defer { GitSignpost.end("patchAndCompare", signpostID) }

        if hints.isUntrackedOrNew {
            return try untrackedOrNewFileDiff(repoPath: repoPath, filePath: filePath, lineLimit: lineLimit)
        }

        if hints.hasStaged == false, hints.hasUnstaged == false {
            return try await resolveAndDiff(
                repoPath: repoPath,
                filePath: filePath,
                lineLimit: lineLimit
            )
        }

        async let stagedTask: GitProcessResult? = hints.hasStaged
            ? GitProcessRunner.runGit(
                repoPath: repoPath,
                arguments: ["-c", "core.quotepath=false", "diff", "--cached", "--no-color", "--no-ext-diff", "--", filePath],
                lineLimit: lineLimit
            )
            : nil

        async let unstagedTask: GitProcessResult? = hints.hasUnstaged
            ? GitProcessRunner.runGit(
                repoPath: repoPath,
                arguments: ["-c", "core.quotepath=false", "diff", "--no-color", "--no-ext-diff", "--", filePath],
                lineLimit: lineLimit
            )
            : nil

        let stagedResult = try await stagedTask
        let unstagedResult = try await unstagedTask

        if let stagedResult, stagedResult.status != 0 {
            throw GitError.commandFailed(stagedResult.stderr.isEmpty ? "Failed to load diff for \(filePath)." : stagedResult.stderr)
        }
        if let unstagedResult, unstagedResult.status != 0 {
            throw GitError.commandFailed(unstagedResult.stderr.isEmpty ? "Failed to load diff for \(filePath)." : unstagedResult.stderr)
        }

        let stagedOut = stagedResult?.stdout ?? ""
        let unstagedOut = unstagedResult?.stdout ?? ""
        let stagedTruncated = stagedResult?.truncated ?? false
        let unstagedTruncated = unstagedResult?.truncated ?? false

        let combinedPatch: String
        let combinedTruncated: Bool
        if !stagedOut.isEmpty, !unstagedOut.isEmpty {
            combinedPatch = stagedOut + "\n" + unstagedOut
            combinedTruncated = stagedTruncated || unstagedTruncated
        } else if !stagedOut.isEmpty {
            combinedPatch = stagedOut
            combinedTruncated = stagedTruncated
        } else {
            combinedPatch = unstagedOut
            combinedTruncated = unstagedTruncated
        }

        return await Self.parsePatchOffMain(combinedPatch, truncated: combinedTruncated)
    }

    func patchAndCompare(
        repoPath: String,
        filePath: String,
        range: DiffRange,
        lineLimit: Int?
    ) async throws -> PatchAndCompareResult {
        try validatePath(repoPath: repoPath, relativePath: filePath)
        try validateRef(range.baseRef)
        try validateRef(range.headRef)

        let result = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["-c", "core.quotepath=false", "diff", "--no-color", "--no-ext-diff", range.spec, "--", filePath],
            lineLimit: lineLimit
        )
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to load diff for \(filePath)." : result.stderr)
        }
        return await Self.parsePatchOffMain(result.stdout, truncated: result.truncated)
    }

    func patchAndCompare(
        repoPath: String,
        filePath: String,
        commit: String,
        lineLimit: Int?
    ) async throws -> PatchAndCompareResult {
        try validateHash(commit)
        return try await patchAndCompare(
            repoPath: repoPath,
            filePath: filePath,
            range: DiffRange(baseRef: "\(commit)^", headRef: commit),
            lineLimit: lineLimit
        )
    }

    func fetchPullRequestDiffHead(repoPath: String, number: Int, remote: String) async throws -> String {
        let localRef = Self.localPullRequestDiffRef(number: number)
        try validateRef(remote)
        try validateRef(localRef)
        let result = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["fetch", remote, "+refs/pull/\(number)/head:\(localRef)"]
        )
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to fetch pull request diff." : result.stderr)
        }
        return localRef
    }

    static func localPullRequestDiffRef(number: Int) -> String {
        "refs/muxy/pull/\(number)/head"
    }

    private static func parseNameStatus(_ data: Data, stats: [String: NumstatEntry]) -> [GitStatusFile] {
        guard let decoded = String(data: data, encoding: .utf8), !decoded.isEmpty else { return [] }
        let tokens = decoded.split(separator: "\0", omittingEmptySubsequences: true).map(String.init)
        var files: [GitStatusFile] = []
        var index = 0

        while index + 1 < tokens.count {
            let status = tokens[index]
            let path = tokens[index + 1]
            let code = status.first ?? "M"

            if code == "R" || code == "C", index + 2 < tokens.count {
                let newPath = tokens[index + 2]
                let stat = stats[newPath]
                files.append(GitStatusFile(
                    path: newPath,
                    oldPath: path,
                    xStatus: code,
                    yStatus: " ",
                    additions: stat?.additions,
                    deletions: stat?.deletions,
                    isBinary: stat?.isBinary ?? false
                ))
                index += 3
            } else {
                let stat = stats[path]
                files.append(GitStatusFile(
                    path: path,
                    oldPath: nil,
                    xStatus: code,
                    yStatus: " ",
                    additions: stat?.additions,
                    deletions: stat?.deletions,
                    isBinary: stat?.isBinary ?? false
                ))
                index += 2
            }
        }

        return files.sorted { $0.path.localizedStandardCompare($1.path) == .orderedAscending }
    }

    static func parsePatchOffMain(_ patch: String, truncated: Bool) async -> PatchAndCompareResult {
        await GitProcessRunner.offMain {
            let parsed = GitDiffParser.parseRows(patch)
            return PatchAndCompareResult(
                rows: GitDiffParser.collapseContextRows(parsed.rows),
                truncated: truncated,
                additions: parsed.additions,
                deletions: parsed.deletions
            )
        }
    }

    private func resolveAndDiff(
        repoPath: String,
        filePath: String,
        lineLimit: Int?
    ) async throws -> PatchAndCompareResult {
        let statusResult = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["-c", "core.quotepath=false", "status", "--porcelain=1", "-z", "--", filePath]
        )
        let statusString = statusResult.stdout.trimmingCharacters(in: CharacterSet(charactersIn: "\0"))

        if statusString.hasPrefix("??") || statusString.hasPrefix("A ") {
            return try untrackedOrNewFileDiff(repoPath: repoPath, filePath: filePath, lineLimit: lineLimit)
        }

        async let stagedTask = GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["-c", "core.quotepath=false", "diff", "--cached", "--no-color", "--no-ext-diff", "--", filePath],
            lineLimit: lineLimit
        )
        async let unstagedTask = GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["-c", "core.quotepath=false", "diff", "--no-color", "--no-ext-diff", "--", filePath],
            lineLimit: lineLimit
        )

        let stagedResult = try await stagedTask
        let unstagedResult = try await unstagedTask

        guard stagedResult.status == 0 else {
            throw GitError.commandFailed(stagedResult.stderr.isEmpty ? "Failed to load diff for \(filePath)." : stagedResult.stderr)
        }
        guard unstagedResult.status == 0 else {
            throw GitError.commandFailed(unstagedResult.stderr.isEmpty ? "Failed to load diff for \(filePath)." : unstagedResult.stderr)
        }

        let combinedPatch: String
        let combinedTruncated: Bool
        if !stagedResult.stdout.isEmpty, !unstagedResult.stdout.isEmpty {
            combinedPatch = stagedResult.stdout + "\n" + unstagedResult.stdout
            combinedTruncated = stagedResult.truncated || unstagedResult.truncated
        } else if !stagedResult.stdout.isEmpty {
            combinedPatch = stagedResult.stdout
            combinedTruncated = stagedResult.truncated
        } else {
            combinedPatch = unstagedResult.stdout
            combinedTruncated = unstagedResult.truncated
        }

        return await Self.parsePatchOffMain(combinedPatch, truncated: combinedTruncated)
    }

    private func untrackedOrNewFileDiff(repoPath: String, filePath: String, lineLimit: Int?) throws -> PatchAndCompareResult {
        let fileURL = URL(fileURLWithPath: repoPath).appendingPathComponent(filePath)
        let resolvedRepo = URL(fileURLWithPath: repoPath).resolvingSymlinksInPath().standardizedFileURL.path
        let resolvedFile = fileURL.resolvingSymlinksInPath().standardizedFileURL.path
        guard resolvedFile.hasPrefix(resolvedRepo + "/") else {
            throw GitError.commandFailed("File path is outside the repository.")
        }
        guard let fileLines = try readDiffPreviewLines(path: fileURL.path, lineLimit: lineLimit) else {
            return PatchAndCompareResult(rows: [], truncated: false, additions: 0, deletions: 0)
        }

        var rows: [DiffDisplayRow] = []
        rows.append(DiffDisplayRow(
            kind: .hunk,
            oldLineNumber: nil,
            newLineNumber: nil,
            oldText: nil,
            newText: nil,
            text: "@@ -0,0 +1,\(fileLines.lines.count) @@ (new file)"
        ))

        for i in 0 ..< fileLines.lines.count {
            let line = fileLines.lines[i]
            rows.append(DiffDisplayRow(
                kind: .addition,
                oldLineNumber: nil,
                newLineNumber: i + 1,
                oldText: nil,
                newText: line,
                text: "+\(line)"
            ))
        }

        return PatchAndCompareResult(
            rows: GitDiffParser.collapseContextRows(rows),
            truncated: fileLines.truncated,
            additions: fileLines.lines.count,
            deletions: 0
        )
    }

    private func readDiffPreviewLines(path: String, lineLimit: Int?) throws -> (lines: [String], truncated: Bool)? {
        guard let handle = FileHandle(forReadingAtPath: path) else { return nil }
        defer { try? handle.close() }

        guard let lineLimit else {
            let data = handle.readDataToEndOfFile()
            guard let content = String(data: data, encoding: .utf8) else { return nil }
            return (content.split(omittingEmptySubsequences: false, whereSeparator: \.isNewline).map(String.init), false)
        }

        var lines: [String] = []
        lines.reserveCapacity(min(lineLimit, 4096))
        var buffer = Data()
        let chunkSize = 65536

        while lines.count < lineLimit {
            let chunk = try handle.read(upToCount: chunkSize) ?? Data()
            if chunk.isEmpty {
                if !buffer.isEmpty, let line = String(data: buffer, encoding: .utf8) {
                    lines.append(line)
                }
                return (lines, false)
            }

            for byte in chunk {
                if byte == 0x0A {
                    guard let line = String(data: buffer, encoding: .utf8) else { return nil }
                    lines.append(line)
                    buffer.removeAll(keepingCapacity: true)
                    if lines.count == lineLimit {
                        return (lines, true)
                    }
                } else {
                    buffer.append(byte)
                }
            }
        }

        return (lines, true)
    }

    func stageFiles(repoPath: String, paths: [String]) async throws {
        for path in paths {
            try validatePath(repoPath: repoPath, relativePath: path)
        }
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["add", "--"] + paths)
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to stage files." : result.stderr)
        }
    }

    func stageAll(repoPath: String) async throws {
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["add", "-A"])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to stage all files." : result.stderr)
        }
    }

    func unstageFiles(repoPath: String, paths: [String]) async throws {
        for path in paths {
            try validatePath(repoPath: repoPath, relativePath: path)
        }
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["reset", "HEAD", "--"] + paths)
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to unstage files." : result.stderr)
        }
    }

    func unstageAll(repoPath: String) async throws {
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["reset", "HEAD"])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to unstage all files." : result.stderr)
        }
    }

    func discardFiles(repoPath: String, paths: [String], untrackedPaths: [String]) async throws {
        for path in paths + untrackedPaths {
            try validatePath(repoPath: repoPath, relativePath: path)
        }

        if !paths.isEmpty {
            let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["checkout", "--"] + paths)
            guard result.status == 0 else {
                throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to discard changes." : result.stderr)
            }
        }

        for relativePath in untrackedPaths {
            let fullPath = (repoPath as NSString).appendingPathComponent(relativePath)
            try FileManager.default.removeItem(atPath: fullPath)
        }
    }

    func discardAll(repoPath: String) async throws {
        let checkoutResult = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["checkout", "--", "."])
        guard checkoutResult.status == 0 else {
            throw GitError.commandFailed(
                checkoutResult.stderr.isEmpty ? "Failed to discard tracked changes." : checkoutResult.stderr
            )
        }

        let cleanResult = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["clean", "-fd"])
        guard cleanResult.status == 0 else {
            throw GitError.commandFailed(
                cleanResult.stderr.isEmpty ? "Failed to clean untracked files." : cleanResult.stderr
            )
        }
    }

    func commit(repoPath: String, message: String) async throws -> String {
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["commit", "-m", message])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to commit." : result.stderr)
        }

        GitMetadataCache.shared.invalidatePRInfo(repoPath: repoPath)

        let hashResult = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["rev-parse", "--short", "HEAD"])
        guard hashResult.status == 0 else { return "" }
        return hashResult.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    func push(repoPath: String) async throws {
        if let result = try await pushPullRequestBranch(repoPath: repoPath) {
            guard result.status == 0 else {
                throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to push." : result.stderr)
            }
            GitMetadataCache.shared.invalidatePRInfo(repoPath: repoPath)
            return
        }

        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["push"])
        guard result.status == 0 else {
            if result.stderr.contains("has no upstream branch") {
                throw GitError.noUpstreamBranch
            }
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to push." : result.stderr)
        }
        GitMetadataCache.shared.invalidatePRInfo(repoPath: repoPath)
    }

    private func pushPullRequestBranch(repoPath: String) async throws -> GitProcessResult? {
        let branch = try await currentBranch(repoPath: repoPath)
        guard await configuredPullRequestNumber(repoPath: repoPath, branch: branch) != nil else { return nil }
        let upstreamResult = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"]
        )
        guard upstreamResult.status == 0 else { return nil }
        let upstream = upstreamResult.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let separator = upstream.firstIndex(of: "/") else { return nil }
        let remote = String(upstream[..<separator])
        let remoteBranch = String(upstream[upstream.index(after: separator)...])
        guard !remote.isEmpty, !remoteBranch.isEmpty else { return nil }
        return try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["push", remote, "HEAD:refs/heads/\(remoteBranch)"]
        )
    }

    func pushSetUpstream(repoPath: String, branch: String) async throws {
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["push", "--set-upstream", "origin", branch])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to push." : result.stderr)
        }
        GitMetadataCache.shared.invalidatePRInfo(repoPath: repoPath, branch: branch)
    }

    func pull(repoPath: String) async throws {
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["pull"])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to pull." : result.stderr)
        }
        GitMetadataCache.shared.invalidatePRInfo(repoPath: repoPath)
    }

    func mergeBaseIntoCurrentBranch(repoPath: String, baseBranch: String) async throws {
        let trimmed = baseBranch.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty,
              trimmed.unicodeScalars.allSatisfy({ Self.allowedBranchCharacters.contains($0) })
        else {
            throw GitError.commandFailed("Invalid base branch name.")
        }

        let statusResult = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["status", "--porcelain=1", "--untracked-files=no"]
        )
        if statusResult.status == 0,
           !statusResult.stdout.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        {
            throw GitError.commandFailed("Commit or stash your changes before updating the branch.")
        }

        let fetchResult = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["fetch", "origin", trimmed]
        )
        guard fetchResult.status == 0 else {
            throw GitError.commandFailed(
                fetchResult.stderr.isEmpty ? "Failed to fetch origin/\(trimmed)." : fetchResult.stderr
            )
        }

        let mergeResult = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["merge", "--no-edit", "origin/\(trimmed)"]
        )
        guard mergeResult.status == 0 else {
            _ = try? await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["merge", "--abort"])
            let detail = mergeResult.stderr.isEmpty ? mergeResult.stdout : mergeResult.stderr
            let trimmedDetail = detail.trimmingCharacters(in: .whitespacesAndNewlines)
            throw GitError.commandFailed(
                trimmedDetail.isEmpty
                    ? "Could not merge origin/\(trimmed) — resolve conflicts manually."
                    : "Could not merge origin/\(trimmed): \(trimmedDetail)"
            )
        }

        let pushResult = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["push"])
        guard pushResult.status == 0 else {
            throw GitError.commandFailed(
                pushResult.stderr.isEmpty ? "Merged locally but failed to push." : pushResult.stderr
            )
        }
        GitMetadataCache.shared.invalidatePRInfo(repoPath: repoPath)
    }

    @discardableResult
    func fastForwardBranch(repoPath: String, branch: String) async -> Bool {
        let trimmed = branch.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty,
              trimmed.unicodeScalars.allSatisfy({ Self.allowedBranchCharacters.contains($0) })
        else { return false }

        let headResult = try? await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["symbolic-ref", "--quiet", "--short", "HEAD"]
        )
        let currentBranch = headResult?.stdout.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""

        let success: Bool
        if currentBranch == trimmed {
            let result = try? await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["pull", "--ff-only"])
            success = result?.status == 0
        } else {
            success = await fastForwardInactiveBranch(repoPath: repoPath, branch: trimmed)
        }
        if success {
            GitMetadataCache.shared.invalidatePRInfo(repoPath: repoPath, branch: trimmed)
        }
        return success
    }

    private func fastForwardInactiveBranch(repoPath: String, branch: String) async -> Bool {
        let localRef = "refs/heads/\(branch)"
        let remoteRef = "refs/remotes/origin/\(branch)"
        let fetchResult = try? await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["fetch", "origin", "refs/heads/\(branch):\(remoteRef)"]
        )
        guard fetchResult?.status == 0 else { return false }

        let localExistsResult = try? await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["show-ref", "--verify", "--quiet", localRef]
        )
        if localExistsResult?.status == 0 {
            let ancestorResult = try? await GitProcessRunner.runGit(
                repoPath: repoPath,
                arguments: ["merge-base", "--is-ancestor", branch, remoteRef]
            )
            guard ancestorResult?.status == 0 else { return false }
        }

        let updateResult = try? await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["update-ref", localRef, remoteRef]
        )
        return updateResult?.status == 0
    }

    func listBranches(repoPath: String) async throws -> [String] {
        let result = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["branch", "--list", "--format=%(refname:short)"]
        )
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to list branches." : result.stderr)
        }
        return result.stdout
            .split(separator: "\n", omittingEmptySubsequences: true)
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .sorted { $0.localizedStandardCompare($1) == .orderedAscending }
    }

    private static let allowedBranchCharacters = CharacterSet.alphanumerics
        .union(CharacterSet(charactersIn: "._/-"))

    func switchBranch(repoPath: String, branch: String) async throws {
        guard !branch.isEmpty,
              branch.unicodeScalars.allSatisfy({ Self.allowedBranchCharacters.contains($0) })
        else {
            throw GitError.commandFailed("Invalid branch name.")
        }
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["switch", branch])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to switch branch." : result.stderr)
        }
    }

    func createAndSwitchBranch(repoPath: String, name: String) async throws {
        let trimmedName = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedName.isEmpty,
              !trimmedName.hasPrefix("-"),
              trimmedName.unicodeScalars.allSatisfy({ Self.allowedBranchCharacters.contains($0) })
        else {
            throw GitError.commandFailed("Invalid branch name.")
        }
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["switch", "-c", trimmedName])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to create branch." : result.stderr)
        }
    }

    func commitLog(repoPath: String, maxCount: Int = 100, skip: Int = 0) async throws -> [GitCommit] {
        let result = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: [
                "log",
                "--decorate=full",
                "--format=\(GitCommitLogParser.logFormat)",
                "--max-count=\(maxCount)",
                "--skip=\(skip)",
            ]
        )
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to load commit history." : result.stderr)
        }
        return GitCommitLogParser.parseCommitLog(result.stdout)
    }

    func cherryPick(repoPath: String, hash: String) async throws {
        try validateHash(hash)
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["cherry-pick", hash])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to cherry-pick." : result.stderr)
        }
    }

    func revert(repoPath: String, hash: String) async throws {
        try validateHash(hash)
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["revert", "--no-commit", hash])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to revert commit." : result.stderr)
        }
    }

    func createBranch(repoPath: String, name: String, startPoint: String) async throws {
        let trimmedName = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedName.isEmpty,
              !trimmedName.hasPrefix("-"),
              trimmedName.unicodeScalars.allSatisfy({ Self.allowedBranchCharacters.contains($0) })
        else {
            throw GitError.commandFailed("Invalid branch name.")
        }
        try validateHash(startPoint)
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["branch", "--", trimmedName, startPoint])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to create branch." : result.stderr)
        }
    }

    private static let allowedTagCharacters = CharacterSet.alphanumerics
        .union(CharacterSet(charactersIn: "._/-"))

    func createTag(repoPath: String, name: String, hash: String) async throws {
        let trimmedName = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedName.isEmpty,
              !trimmedName.hasPrefix("-"),
              trimmedName.unicodeScalars.allSatisfy({ Self.allowedTagCharacters.contains($0) })
        else {
            throw GitError.commandFailed("Invalid tag name.")
        }
        try validateHash(hash)
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["tag", "--", trimmedName, hash])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to create tag." : result.stderr)
        }
    }

    func checkoutDetached(repoPath: String, hash: String) async throws {
        try validateHash(hash)
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["checkout", "--detach", hash])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to checkout." : result.stderr)
        }
    }

    private static let hexCharacters = CharacterSet(charactersIn: "0123456789abcdefABCDEF")

    private static let refCharacters = CharacterSet.alphanumerics
        .union(CharacterSet(charactersIn: "._/@-^"))

    private func validateHash(_ hash: String) throws {
        guard !hash.isEmpty,
              hash.count <= 40,
              hash.unicodeScalars.allSatisfy({ Self.hexCharacters.contains($0) })
        else {
            throw GitError.commandFailed("Invalid commit hash.")
        }
    }

    private func validateRef(_ ref: String) throws {
        guard !ref.isEmpty,
              !ref.hasPrefix("-"),
              !ref.contains(".."),
              !ref.contains(" "),
              ref.unicodeScalars.allSatisfy({ Self.refCharacters.contains($0) })
        else {
            throw GitError.commandFailed("Invalid Git ref.")
        }
    }

    private func validatePath(repoPath: String, relativePath: String) throws {
        let fullPath = (repoPath as NSString).appendingPathComponent(relativePath)
        let resolvedRepo = (repoPath as NSString).standardizingPath
        let resolvedFull = (fullPath as NSString).standardizingPath
        guard resolvedFull.hasPrefix(resolvedRepo + "/") else {
            throw GitError.commandFailed("File path is outside the repository.")
        }
    }
}
