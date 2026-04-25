import Foundation

struct GitRepositoryService {
    struct PatchAndCompareResult {
        let rows: [DiffDisplayRow]
        let truncated: Bool
        let additions: Int
        let deletions: Int
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
    }

    struct PRListItem: Equatable, Identifiable {
        let number: Int
        let title: String
        let author: String
        let headBranch: String
        let baseBranch: String
        let state: PRState
        let isDraft: Bool
        let url: String
        let updatedAt: Date?
        let checks: PRChecks

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

    func cachedPullRequestInfo(
        repoPath: String,
        branch: String,
        headSha: String,
        forceFresh: Bool
    ) async -> PRInfo? {
        if !forceFresh, let cached = GitMetadataCache.shared.cachedPRInfo(
            repoPath: repoPath,
            branch: branch,
            headSha: headSha
        ) {
            return cached
        }
        let info = await pullRequestInfo(repoPath: repoPath, branch: branch)
        GitMetadataCache.shared.storePRInfo(info, repoPath: repoPath, branch: branch, headSha: headSha)
        return info
    }

    func pullRequestInfo(repoPath: String, branch: String) async -> PRInfo? {
        guard let ghPath = GitProcessRunner.resolveExecutable("gh") else { return nil }
        let jsonFields = "url,number,state,isDraft,baseRefName,mergeable,mergeStateStatus,statusCheckRollup"

        if let info = await ghPRView(ghPath: ghPath, repoPath: repoPath, jsonFields: jsonFields) {
            return info
        }
        return await ghPRView(
            ghPath: ghPath, repoPath: repoPath, branch: branch, jsonFields: jsonFields
        )
    }

    private func ghPRView(
        ghPath: String,
        repoPath: String,
        branch: String? = nil,
        jsonFields: String
    ) async -> PRInfo? {
        var arguments = ["pr", "view"]
        if let branch {
            arguments.append(branch)
        }
        arguments += ["--json", jsonFields]

        let result = try? await GitProcessRunner.runCommand(
            executable: ghPath,
            arguments: arguments,
            workingDirectory: repoPath
        )
        guard let result, result.status == 0 else { return nil }
        return GitPRParser.parsePRInfo(result.stdout)
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
        let jsonFields = "number,title,author,headRefName,baseRefName,state,isDraft,url,updatedAt,statusCheckRollup"
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

    func checkoutPullRequest(repoPath: String, number: Int) async throws {
        guard let ghPath = GitProcessRunner.resolveExecutable("gh") else {
            throw PRCreateError.ghNotInstalled
        }
        let result = try await GitProcessRunner.runCommand(
            executable: ghPath,
            arguments: ["pr", "checkout", String(number)],
            workingDirectory: repoPath
        )
        guard result.status == 0 else {
            let message = result.stderr.isEmpty ? result.stdout : result.stderr
            throw PRCreateError.commandFailed(
                message.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                    ? "Failed to checkout pull request."
                    : message.trimmingCharacters(in: .whitespacesAndNewlines)
            )
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

        if let info = await pullRequestInfo(repoPath: repoPath, branch: branch) {
            return info
        }

        let fallbackURL = createResult.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
        throw PRCreateError.commandFailed(
            fallbackURL.isEmpty
                ? "Pull request created but could not be read back."
                : "Pull request created at \(fallbackURL) but could not be read back."
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

        let verifyResult = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["rev-parse", "--is-inside-work-tree"]
        )
        guard verifyResult.status == 0,
              verifyResult.stdout.trimmingCharacters(in: .whitespacesAndNewlines) == "true"
        else {
            throw GitError.notGitRepository
        }

        async let statusTask = GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["-c", "core.quotepath=false", "status", "--porcelain=1", "-z", "--untracked-files=all"]
        )
        async let numstatTask = GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["-c", "core.quotepath=false", "diff", "HEAD", "--numstat", "--no-color", "--no-ext-diff"]
        )

        let statusResult = try await statusTask
        guard statusResult.status == 0 else {
            _ = try? await numstatTask
            throw GitError.commandFailed(statusResult.stderr.isEmpty ? "Failed to load Git status." : statusResult.stderr)
        }

        let numstatResult = try await numstatTask
        let stats = numstatResult.status == 0 ? GitStatusParser.parseNumstat(numstatResult.stdout) : [:]

        return GitStatusParser.parseStatusPorcelain(statusResult.stdoutData, stats: stats).map { file in
            guard file.additions == nil, file.xStatus == "?" || file.xStatus == "A" else { return file }
            let lineCount = Self.countLines(repoPath: repoPath, relativePath: file.path)
            return GitStatusFile(
                path: file.path,
                oldPath: file.oldPath,
                xStatus: file.xStatus,
                yStatus: file.yStatus,
                additions: lineCount,
                deletions: 0,
                isBinary: file.isBinary
            )
        }
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

    private static func parsePatchOffMain(_ patch: String, truncated: Bool) async -> PatchAndCompareResult {
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
        let fullPath = (repoPath as NSString).appendingPathComponent(filePath)
        let resolvedRepo = (repoPath as NSString).standardizingPath
        let resolvedFull = (fullPath as NSString).standardizingPath
        guard resolvedFull.hasPrefix(resolvedRepo + "/") else {
            throw GitError.commandFailed("File path is outside the repository.")
        }
        guard let data = FileManager.default.contents(atPath: fullPath),
              let content = String(data: data, encoding: .utf8)
        else {
            return PatchAndCompareResult(rows: [], truncated: false, additions: 0, deletions: 0)
        }

        let lines = content.split(omittingEmptySubsequences: false, whereSeparator: \.isNewline)
        let effectiveLines = lineLimit.map { min(lines.count, $0) } ?? lines.count
        let truncated = lineLimit.map { lines.count > $0 } ?? false

        var rows: [DiffDisplayRow] = []
        rows.append(DiffDisplayRow(
            kind: .hunk,
            oldLineNumber: nil,
            newLineNumber: nil,
            oldText: nil,
            newText: nil,
            text: "@@ -0,0 +1,\(lines.count) @@ (new file)"
        ))

        for i in 0 ..< effectiveLines {
            let line = String(lines[i])
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
            truncated: truncated,
            additions: effectiveLines,
            deletions: 0
        )
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
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: ["push"])
        guard result.status == 0 else {
            if result.stderr.contains("has no upstream branch") {
                throw GitError.noUpstreamBranch
            }
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to push." : result.stderr)
        }
        GitMetadataCache.shared.invalidatePRInfo(repoPath: repoPath)
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

    private func validateHash(_ hash: String) throws {
        guard !hash.isEmpty,
              hash.count <= 40,
              hash.unicodeScalars.allSatisfy({ Self.hexCharacters.contains($0) })
        else {
            throw GitError.commandFailed("Invalid commit hash.")
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
