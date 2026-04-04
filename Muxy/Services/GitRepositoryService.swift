import Foundation

struct NumstatEntry {
    let additions: Int?
    let deletions: Int?
    let isBinary: Bool
}

struct GitStatusFile: Identifiable, Hashable {
    let path: String
    let oldPath: String?
    let xStatus: Character
    let yStatus: Character
    let additions: Int?
    let deletions: Int?
    let isBinary: Bool

    var id: String { path }

    var statusText: String {
        switch (xStatus, yStatus) {
        case ("A", _),
             (_, "A"):
            "A"
        case ("D", _),
             (_, "D"):
            "D"
        case ("R", _),
             (_, "R"):
            "R"
        case ("C", _),
             (_, "C"):
            "C"
        case ("M", _),
             (_, "M"):
            "M"
        case ("U", _),
             (_, "U"):
            "U"
        default:
            "?"
        }
    }
}

struct DiffDisplayRow: Identifiable {
    enum Kind {
        case hunk
        case context
        case addition
        case deletion
        case collapsed
    }

    let id = UUID()
    let kind: Kind
    let oldLineNumber: Int?
    let newLineNumber: Int?
    let oldText: String?
    let newText: String?
    let text: String
}

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

    func currentBranch(repoPath: String) async throws -> String {
        let result = try runGit(repoPath: repoPath, arguments: ["rev-parse", "--abbrev-ref", "HEAD"])
        guard result.status == 0 else {
            throw GitError.commandFailed("Failed to get current branch.")
        }
        return result.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    struct PRInfo {
        let url: String
        let number: Int
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

    nonisolated private func resolveExecutable(_ name: String) -> String? {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/which")
        process.arguments = [name]
        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = Pipe()
        do {
            try process.run()
            process.waitUntilExit()
            guard process.terminationStatus == 0 else { return nil }
            let data = pipe.fileHandleForReading.readDataToEndOfFile()
            let path = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines)
            return path?.isEmpty == false ? path : nil
        } catch {
            return nil
        }
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
        let stats = parseNumstat(numstatResult.stdout)

        return parseStatusPorcelain(statusResult.stdoutData, stats: stats).map { file in
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

        let parsed = parseRows(combinedPatch)
        return PatchAndCompareResult(
            rows: collapseContextRows(parsed.rows),
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
            rows: collapseContextRows(rows),
            truncated: truncated,
            additions: effectiveLines,
            deletions: 0
        )
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
        let limit = lineLimit
        return try runGitSync(arguments: args, lineLimit: limit)
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

    private func parseStatusPorcelain(
        _ data: Data,
        stats: [String: NumstatEntry]
    ) -> [GitStatusFile] {
        guard let decoded = String(data: data, encoding: .utf8), !decoded.isEmpty else { return [] }
        let tokens = decoded.split(separator: "\0", omittingEmptySubsequences: true).map(String.init)
        var files: [GitStatusFile] = []
        var index = 0

        while index < tokens.count {
            let token = tokens[index]
            guard token.count >= 4 else {
                index += 1
                continue
            }
            let marker = Array(token)
            let xStatus = marker[0]
            let yStatus = marker[1]
            let path = String(token.dropFirst(3))

            if xStatus == "R" || xStatus == "C" {
                let newPath = index + 1 < tokens.count ? tokens[index + 1] : path
                let stat = stats[newPath]
                files.append(GitStatusFile(
                    path: newPath,
                    oldPath: path,
                    xStatus: xStatus,
                    yStatus: yStatus,
                    additions: stat?.additions,
                    deletions: stat?.deletions,
                    isBinary: stat?.isBinary ?? false
                ))
                index += 2
                continue
            }

            let stat = stats[path]
            files.append(GitStatusFile(
                path: path,
                oldPath: nil,
                xStatus: xStatus,
                yStatus: yStatus,
                additions: stat?.additions,
                deletions: stat?.deletions,
                isBinary: stat?.isBinary ?? false
            ))
            index += 1
        }

        return files.sorted { $0.path.localizedStandardCompare($1.path) == .orderedAscending }
    }

    private func parseNumstat(_ output: String) -> [String: NumstatEntry] {
        var stats: [String: NumstatEntry] = [:]

        for line in output.split(separator: "\n", omittingEmptySubsequences: true) {
            let fields = line.split(separator: "\t", maxSplits: 2, omittingEmptySubsequences: false)
            guard fields.count == 3 else { continue }

            let addsToken = String(fields[0])
            let delsToken = String(fields[1])
            let rawPath = String(fields[2])

            let entry = NumstatEntry(
                additions: Int(addsToken),
                deletions: Int(delsToken),
                isBinary: addsToken == "-" || delsToken == "-"
            )

            let normalizedPath = normalizeNumstatPath(rawPath)
            stats[normalizedPath] = entry
            stats[rawPath] = entry
        }

        return stats
    }

    private func normalizeNumstatPath(_ rawPath: String) -> String {
        if let braceStart = rawPath.firstIndex(of: "{"),
           let braceEnd = rawPath.lastIndex(of: "}"),
           let arrowRange = rawPath.range(of: " => ")
        {
            let prefix = rawPath[..<braceStart]
            let suffix = rawPath[rawPath.index(after: braceEnd)...]
            let right = rawPath[arrowRange.upperBound ..< braceEnd]
            return String(prefix) + String(right) + String(suffix)
        }
        if let arrowRange = rawPath.range(of: " => ") {
            return String(rawPath[arrowRange.upperBound...])
        }
        return rawPath
    }

    struct ParsedDiffRows {
        let rows: [DiffDisplayRow]
        let additions: Int
        let deletions: Int
    }

    private func parseRows(_ patch: String) -> ParsedDiffRows {
        var rows: [DiffDisplayRow] = []
        var oldLineNumber = 0
        var newLineNumber = 0
        var inHunk = false
        var additions = 0
        var deletions = 0

        for rawLine in patch.split(omittingEmptySubsequences: false, whereSeparator: \.isNewline) {
            let line = String(rawLine)

            if line.hasPrefix("@@") {
                inHunk = true
                let (oldStart, newStart) = parseHunkHeader(line)
                oldLineNumber = oldStart
                newLineNumber = newStart
                rows.append(DiffDisplayRow(
                    kind: .hunk,
                    oldLineNumber: nil,
                    newLineNumber: nil,
                    oldText: nil,
                    newText: nil,
                    text: line
                ))
                continue
            }

            guard inHunk else { continue }

            if line.hasPrefix(" ") {
                let content = String(line.dropFirst())
                rows.append(DiffDisplayRow(
                    kind: .context,
                    oldLineNumber: oldLineNumber,
                    newLineNumber: newLineNumber,
                    oldText: content,
                    newText: content,
                    text: " \(content)"
                ))
                oldLineNumber += 1
                newLineNumber += 1
                continue
            }

            if line.hasPrefix("-") {
                let content = String(line.dropFirst())
                rows.append(DiffDisplayRow(
                    kind: .deletion,
                    oldLineNumber: oldLineNumber,
                    newLineNumber: nil,
                    oldText: content,
                    newText: nil,
                    text: "-\(content)"
                ))
                oldLineNumber += 1
                deletions += 1
                continue
            }

            if line.hasPrefix("+") {
                let content = String(line.dropFirst())
                rows.append(DiffDisplayRow(
                    kind: .addition,
                    oldLineNumber: nil,
                    newLineNumber: newLineNumber,
                    oldText: nil,
                    newText: content,
                    text: "+\(content)"
                ))
                newLineNumber += 1
                additions += 1
                continue
            }
        }

        return ParsedDiffRows(rows: rows, additions: additions, deletions: deletions)
    }

    private func collapseContextRows(_ rows: [DiffDisplayRow]) -> [DiffDisplayRow] {
        var output: [DiffDisplayRow] = []
        var index = 0
        let leadingContext = 3
        let trailingContext = 3
        let collapseThreshold = 12

        while index < rows.count {
            let row = rows[index]
            if row.kind != .context {
                output.append(row)
                index += 1
                continue
            }

            var end = index
            while end < rows.count, rows[end].kind == .context {
                end += 1
            }
            let runLength = end - index

            if runLength <= collapseThreshold {
                output.append(contentsOf: rows[index ..< end])
            } else {
                let startKeepEnd = index + leadingContext
                let endKeepStart = end - trailingContext
                output.append(contentsOf: rows[index ..< startKeepEnd])
                output.append(DiffDisplayRow(
                    kind: .collapsed,
                    oldLineNumber: nil,
                    newLineNumber: nil,
                    oldText: nil,
                    newText: nil,
                    text: "\(runLength - leadingContext - trailingContext) unmodified lines"
                ))
                output.append(contentsOf: rows[endKeepStart ..< end])
            }
            index = end
        }

        return output
    }

    private func parseHunkHeader(_ line: String) -> (Int, Int) {
        let parts = line.split(separator: " ")
        guard parts.count >= 3 else { return (0, 0) }

        let oldPart = String(parts[1])
        let newPart = String(parts[2])

        let oldNumber = parseHunkNumber(oldPart)
        let newNumber = parseHunkNumber(newPart)
        return (oldNumber, newNumber)
    }

    private func parseHunkNumber(_ token: String) -> Int {
        let cleaned = token.trimmingCharacters(in: CharacterSet(charactersIn: "-+,"))
        guard let start = cleaned.split(separator: ",").first else { return 0 }
        return Int(start) ?? 0
    }
}
