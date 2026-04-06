import Foundation

actor GitRepositoryService {
    struct PatchAndCompareResult {
        let rows: [DiffDisplayRow]
        let truncated: Bool
        let additions: Int
        let deletions: Int
    }

    enum GitError: LocalizedError {
        case notGitRepository
        case commandFailed(String)

        var errorDescription: String? {
            switch self {
            case .notGitRepository:
                "This folder is not a Git repository."
            case let .commandFailed(message):
                message
            }
        }
    }

    struct PRInfo {
        let url: String
        let number: Int
    }

    func currentBranch(repoPath: String) async throws -> String {
        let result = try runGit(repoPath: repoPath, arguments: ["rev-parse", "--abbrev-ref", "HEAD"])
        guard result.status == 0 else {
            throw GitError.commandFailed("Failed to get current branch.")
        }
        return result.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    func pullRequestInfo(repoPath: String, branch: String) async -> PRInfo? {
        guard let ghPath = resolveExecutable("gh") else { return nil }
        let result = try? runCommand(
            executable: ghPath,
            arguments: ["pr", "view", branch, "--json", "url,number", "-q", ".url + \"\\n\" + (.number | tostring)"],
            workingDirectory: repoPath
        )
        guard let result, result.status == 0 else { return nil }
        let lines = result.stdout.trimmingCharacters(in: .whitespacesAndNewlines).split(separator: "\n")
        guard lines.count >= 2,
              let number = Int(lines[1])
        else { return nil }
        return PRInfo(url: String(lines[0]), number: number)
    }

    func changedFiles(repoPath: String, ignoreWhitespace: Bool = false) async throws -> [GitStatusFile] {
        let result = try runGit(repoPath: repoPath, arguments: ["rev-parse", "--is-inside-work-tree"])
        guard result.status == 0, result.stdout.trimmingCharacters(in: .whitespacesAndNewlines) == "true" else {
            throw GitError.notGitRepository
        }

        let statusResult = try runGit(
            repoPath: repoPath,
            arguments: ["-c", "core.quotepath=false", "status", "--porcelain=1", "-z", "--untracked-files=all"]
        )
        guard statusResult.status == 0 else {
            throw GitError.commandFailed(statusResult.stderr.isEmpty ? "Failed to load Git status." : statusResult.stderr)
        }

        let wsFlag = ignoreWhitespace ? ["-w"] : [String]()
        let numstatResult = try runGit(
            repoPath: repoPath,
            arguments: ["-c", "core.quotepath=false", "diff", "--numstat", "--no-color", "--no-ext-diff"] + wsFlag
        )
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
        ignoreWhitespace: Bool = false
    ) async throws -> PatchAndCompareResult {
        let statusResult = try runGit(
            repoPath: repoPath,
            arguments: ["-c", "core.quotepath=false", "status", "--porcelain=1", "-z", "--", filePath]
        )
        let statusString = statusResult.stdout.trimmingCharacters(in: CharacterSet(charactersIn: "\0"))

        if statusString.hasPrefix("??") || statusString.hasPrefix("A ") {
            return try untrackedOrNewFileDiff(repoPath: repoPath, filePath: filePath, lineLimit: lineLimit)
        }

        let wsFlag = ignoreWhitespace ? ["-w"] : [String]()

        let stagedResult = try runGit(
            repoPath: repoPath,
            arguments: ["-c", "core.quotepath=false", "diff", "--cached", "--no-color", "--no-ext-diff"] + wsFlag + ["--", filePath],
            lineLimit: lineLimit
        )
        guard stagedResult.status == 0 else {
            throw GitError.commandFailed(stagedResult.stderr.isEmpty ? "Failed to load diff for \(filePath)." : stagedResult.stderr)
        }

        let unstagedResult = try runGit(
            repoPath: repoPath,
            arguments: ["-c", "core.quotepath=false", "diff", "--no-color", "--no-ext-diff"] + wsFlag + ["--", filePath],
            lineLimit: lineLimit
        )
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

        let parsed = GitDiffParser.parseRows(combinedPatch)
        return PatchAndCompareResult(
            rows: GitDiffParser.collapseContextRows(parsed.rows),
            truncated: combinedTruncated,
            additions: parsed.additions,
            deletions: parsed.deletions
        )
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
        let result = try runGit(repoPath: repoPath, arguments: ["add", "--"] + paths)
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to stage files." : result.stderr)
        }
    }

    func stageAll(repoPath: String) async throws {
        let result = try runGit(repoPath: repoPath, arguments: ["add", "-A"])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to stage all files." : result.stderr)
        }
    }

    func unstageFiles(repoPath: String, paths: [String]) async throws {
        for path in paths {
            try validatePath(repoPath: repoPath, relativePath: path)
        }
        let result = try runGit(repoPath: repoPath, arguments: ["reset", "HEAD", "--"] + paths)
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to unstage files." : result.stderr)
        }
    }

    func unstageAll(repoPath: String) async throws {
        let result = try runGit(repoPath: repoPath, arguments: ["reset", "HEAD"])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to unstage all files." : result.stderr)
        }
    }

    func discardFiles(repoPath: String, paths: [String], untrackedPaths: [String]) async throws {
        for path in paths + untrackedPaths {
            try validatePath(repoPath: repoPath, relativePath: path)
        }

        if !paths.isEmpty {
            let result = try runGit(repoPath: repoPath, arguments: ["checkout", "--"] + paths)
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
        let checkoutResult = try runGit(repoPath: repoPath, arguments: ["checkout", "--", "."])
        guard checkoutResult.status == 0 else {
            throw GitError.commandFailed(
                checkoutResult.stderr.isEmpty ? "Failed to discard tracked changes." : checkoutResult.stderr
            )
        }

        let cleanResult = try runGit(repoPath: repoPath, arguments: ["clean", "-fd"])
        guard cleanResult.status == 0 else {
            throw GitError.commandFailed(
                cleanResult.stderr.isEmpty ? "Failed to clean untracked files." : cleanResult.stderr
            )
        }
    }

    func commit(repoPath: String, message: String) async throws -> String {
        let result = try runGit(repoPath: repoPath, arguments: ["commit", "-m", message])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to commit." : result.stderr)
        }

        let hashResult = try runGit(repoPath: repoPath, arguments: ["rev-parse", "--short", "HEAD"])
        guard hashResult.status == 0 else { return "" }
        return hashResult.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    func push(repoPath: String) async throws {
        let result = try runGit(repoPath: repoPath, arguments: ["push"])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to push." : result.stderr)
        }
    }

    func pull(repoPath: String) async throws {
        let result = try runGit(repoPath: repoPath, arguments: ["pull"])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to pull." : result.stderr)
        }
    }

    func listBranches(repoPath: String) async throws -> [String] {
        let result = try runGit(
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
        let result = try runGit(repoPath: repoPath, arguments: ["switch", branch])
        guard result.status == 0 else {
            throw GitError.commandFailed(result.stderr.isEmpty ? "Failed to switch branch." : result.stderr)
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

    private struct GitRunResult {
        let status: Int32
        let stdout: String
        let stdoutData: Data
        let stderr: String
        let truncated: Bool
    }

    private func runGit(repoPath: String, arguments: [String], lineLimit: Int? = nil) throws -> GitRunResult {
        let args = ["git", "-C", repoPath] + arguments
        return try runGitSync(arguments: args, lineLimit: lineLimit)
    }

    private static let searchPaths = [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/usr/bin",
        "/bin",
        "/usr/sbin",
        "/sbin",
    ]

    nonisolated private func resolveExecutable(_ name: String) -> String? {
        for directory in Self.searchPaths {
            let path = "\(directory)/\(name)"
            if FileManager.default.isExecutableFile(atPath: path) {
                return path
            }
        }
        return nil
    }

    nonisolated private func runCommand(
        executable: String,
        arguments: [String],
        workingDirectory: String
    ) throws -> GitRunResult {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: executable)
        process.arguments = arguments
        process.currentDirectoryURL = URL(fileURLWithPath: workingDirectory)

        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe

        try process.run()

        let stdoutData = stdoutPipe.fileHandleForReading.readDataToEndOfFile()
        let stderrData = stderrPipe.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()

        let stdout = String(data: stdoutData, encoding: .utf8) ?? ""
        let stderr = String(data: stderrData, encoding: .utf8) ?? ""
        return GitRunResult(status: process.terminationStatus, stdout: stdout, stdoutData: stdoutData, stderr: stderr, truncated: false)
    }

    nonisolated private func runGitSync(arguments: [String], lineLimit: Int?) throws -> GitRunResult {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = arguments

        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe

        try process.run()

        let stdoutData: Data = if let lineLimit {
            try readWithLineLimit(handle: stdoutPipe.fileHandleForReading, process: process, lineLimit: lineLimit)
        } else {
            stdoutPipe.fileHandleForReading.readDataToEndOfFile()
        }
        let stderrData = stderrPipe.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()

        let stdout = String(data: stdoutData, encoding: .utf8) ?? ""
        let stderr = String(data: stderrData, encoding: .utf8) ?? ""
        let truncated = process.terminationReason == .uncaughtSignal
        return GitRunResult(status: process.terminationStatus, stdout: stdout, stdoutData: stdoutData, stderr: stderr, truncated: truncated)
    }

    nonisolated private func readWithLineLimit(handle: FileHandle, process: Process, lineLimit: Int) throws -> Data {
        var collected = Data()
        var currentLineCount = 0
        let chunkSize = 65536

        while true {
            let chunk = try handle.read(upToCount: chunkSize) ?? Data()
            if chunk.isEmpty {
                return collected
            }

            collected.append(chunk)
            currentLineCount += chunk.reduce(into: 0) { count, byte in
                if byte == 0x0A { count += 1 }
            }

            if currentLineCount >= lineLimit {
                process.terminate()
                return collected
            }
        }
    }
}
