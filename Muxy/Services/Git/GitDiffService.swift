import Foundation

struct GitDiffService {
    struct PatchAndCompareResult {
        let rows: [DiffDisplayRow]
        let truncated: Bool
        let additions: Int
        let deletions: Int
    }

    struct DiffHints {
        let hasStaged: Bool
        let hasUnstaged: Bool
        let isUntrackedOrNew: Bool

        static let unknown = DiffHints(hasStaged: true, hasUnstaged: true, isUntrackedOrNew: false)
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
            arguments: ["-c", "core.quotepath=false", "diff", "--numstat", "--no-color", "--no-ext-diff"]
        )

        let statusResult = try await statusTask
        guard statusResult.status == 0 else {
            _ = try? await numstatTask
            throw GitError.commandFailed(statusResult.stderr.isEmpty ? "Failed to load Git status." : statusResult.stderr)
        }

        let numstatResult = try await numstatTask
        let stats = GitStatusParser.parseNumstat(numstatResult.stdout)

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

    private static func countLines(repoPath: String, relativePath: String) -> Int? {
        let fullPath = (repoPath as NSString).appendingPathComponent(relativePath)
        guard let data = FileManager.default.contents(atPath: fullPath),
              let content = String(data: data, encoding: .utf8)
        else {
            return nil
        }
        return content.isEmpty ? 0 : content.split(omittingEmptySubsequences: false, whereSeparator: \.isNewline).count
    }
}
