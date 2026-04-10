import Foundation

struct FileSearchResult: Identifiable, Equatable {
    let id: String
    let relativePath: String
    let absolutePath: String
    let fileName: String
}

enum FileSearchService {
    static let maxResults = 30
    private static let candidatePoolLimit = 500
    private static let initialMaxDepth = 4
    private static let initialCandidateLimit = 150

    private static let prunedDirectoryNames: [String] = [
        ".git", "node_modules", ".build", "build", "DerivedData",
        "__pycache__", ".venv", "venv", "dist", ".next", ".nuxt",
        "target", "Pods", ".swiftpm", ".idea", ".vscode",
        "vendor", "coverage", ".cache", ".parcel-cache",
    ]

    static func search(query: String, in projectPath: String) async -> [FileSearchResult] {
        let trimmed = query.trimmingCharacters(in: .whitespaces)

        if trimmed.isEmpty {
            let candidates = await runFind(
                arguments: initialArguments(projectPath: projectPath),
                projectPath: projectPath,
                limit: initialCandidateLimit
            )
            return rankInitialCandidates(candidates)
        }

        let candidates = await runFind(
            arguments: queryArguments(query: trimmed, projectPath: projectPath),
            projectPath: projectPath,
            limit: candidatePoolLimit
        )
        return rankCandidates(candidates, query: trimmed)
    }

    private static func runFind(
        arguments: [String],
        projectPath: String,
        limit: Int
    ) async -> [FileSearchResult] {
        await withCheckedContinuation { continuation in
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/usr/bin/find")
            process.arguments = arguments

            let stdoutPipe = Pipe()
            let stderrPipe = Pipe()
            process.standardOutput = stdoutPipe
            process.standardError = stderrPipe

            let resultsBox = ResultsBox()
            let handle = stdoutPipe.fileHandleForReading

            handle.readabilityHandler = { fileHandle in
                let data = fileHandle.availableData
                if data.isEmpty { return }
                guard let chunk = String(data: data, encoding: .utf8) else { return }
                let done = resultsBox.append(chunk: chunk, projectPath: projectPath, limit: limit)
                if done, process.isRunning {
                    process.terminate()
                }
            }

            process.terminationHandler = { _ in
                handle.readabilityHandler = nil
                if let remaining = try? handle.readToEnd(), let chunk = String(data: remaining, encoding: .utf8) {
                    _ = resultsBox.append(chunk: chunk, projectPath: projectPath, limit: limit)
                }
                continuation.resume(returning: resultsBox.take())
            }

            do {
                try process.run()
            } catch {
                handle.readabilityHandler = nil
                continuation.resume(returning: [])
            }
        }
    }

    private static func initialArguments(projectPath: String) -> [String] {
        var args: [String] = [projectPath, "-maxdepth", "\(initialMaxDepth)"]
        appendPruneClause(into: &args)
        args.append(contentsOf: ["-type", "f", "-print"])
        return args
    }

    private static func queryArguments(query: String, projectPath: String) -> [String] {
        var args: [String] = [projectPath]
        appendPruneClause(into: &args)
        args.append(contentsOf: ["-type", "f", "-ipath", fuzzyGlob(for: query), "-print"])
        return args
    }

    private static func appendPruneClause(into args: inout [String]) {
        guard !prunedDirectoryNames.isEmpty else { return }
        args.append("(")
        for (index, name) in prunedDirectoryNames.enumerated() {
            if index > 0 { args.append("-o") }
            args.append("-name")
            args.append(name)
        }
        args.append(")")
        args.append("-prune")
        args.append("-o")
    }

    private static func fuzzyGlob(for query: String) -> String {
        var glob = "*"
        for character in query where !character.isWhitespace && character != "/" {
            glob.append(character)
            glob.append("*")
        }
        return glob
    }

    private static func rankInitialCandidates(_ candidates: [FileSearchResult]) -> [FileSearchResult] {
        candidates
            .sorted { lhs, rhs in
                if lhs.relativePath.count != rhs.relativePath.count {
                    return lhs.relativePath.count < rhs.relativePath.count
                }
                return lhs.relativePath.localizedCaseInsensitiveCompare(rhs.relativePath) == .orderedAscending
            }
            .prefix(maxResults)
            .map(\.self)
    }

    private static func rankCandidates(_ candidates: [FileSearchResult], query: String) -> [FileSearchResult] {
        let queryChars = Array(query.lowercased())
        guard !queryChars.isEmpty else { return [] }
        let isPathQuery = query.contains("/")

        var scored: [(result: FileSearchResult, score: Int)] = []
        scored.reserveCapacity(candidates.count)

        for candidate in candidates {
            guard let score = FuzzyScorer.score(
                queryLower: queryChars,
                fileName: candidate.fileName,
                relativePath: candidate.relativePath,
                preferPath: isPathQuery
            )
            else { continue }
            scored.append((candidate, score))
        }

        scored.sort { lhs, rhs in
            if lhs.score != rhs.score { return lhs.score > rhs.score }
            if lhs.result.relativePath.count != rhs.result.relativePath.count {
                return lhs.result.relativePath.count < rhs.result.relativePath.count
            }
            return lhs.result.relativePath.localizedCaseInsensitiveCompare(rhs.result.relativePath) == .orderedAscending
        }

        return scored.prefix(maxResults).map(\.result)
    }
}

private enum FuzzyScorer {
    static func score(
        queryLower: [Character],
        fileName: String,
        relativePath: String,
        preferPath: Bool
    ) -> Int? {
        let pathLower = Array(relativePath.lowercased())
        let pathOriginal = Array(relativePath)

        if preferPath {
            guard let pathScore = scoreAgainst(
                queryLower: queryLower,
                targetLower: pathLower,
                targetOriginal: pathOriginal
            )
            else { return nil }
            return pathScore.score + 1000
        }

        let fileNameLower = Array(fileName.lowercased())
        if let nameScore = scoreAgainst(
            queryLower: queryLower,
            targetLower: fileNameLower,
            targetOriginal: Array(fileName)
        ) {
            let bonus = fileNamePositionBonus(nameScore: nameScore, fileNameLength: fileNameLower.count)
            return nameScore.score + bonus + 1000
        }

        if let pathScore = scoreAgainst(
            queryLower: queryLower,
            targetLower: pathLower,
            targetOriginal: pathOriginal
        ) {
            return pathScore.score
        }

        return nil
    }

    private struct SubsequenceScore {
        let score: Int
        let firstMatchIndex: Int
    }

    private static func scoreAgainst(
        queryLower: [Character],
        targetLower: [Character],
        targetOriginal: [Character]
    ) -> SubsequenceScore? {
        guard !queryLower.isEmpty, targetLower.count >= queryLower.count else { return nil }

        if queryLower.count == targetLower.count, queryLower == targetLower {
            return SubsequenceScore(score: 10000, firstMatchIndex: 0)
        }

        if let range = rangeOfContiguous(query: queryLower, in: targetLower) {
            var score = 5000 - range.lowerBound * 10
            if range.lowerBound == 0 { score += 1500 }
            if range.lowerBound > 0, isSeparator(targetLower[range.lowerBound - 1]) {
                score += 800
            }
            return SubsequenceScore(score: score, firstMatchIndex: range.lowerBound)
        }

        return subsequenceScore(
            queryLower: queryLower,
            targetLower: targetLower,
            targetOriginal: targetOriginal
        )
    }

    private static func rangeOfContiguous(query: [Character], in target: [Character]) -> Range<Int>? {
        guard query.count <= target.count else { return nil }
        let lastStart = target.count - query.count
        if lastStart < 0 { return nil }
        for start in 0 ... lastStart {
            var matched = true
            for offset in 0 ..< query.count where target[start + offset] != query[offset] {
                matched = false
                break
            }
            if matched { return start ..< (start + query.count) }
        }
        return nil
    }

    private static func subsequenceScore(
        queryLower: [Character],
        targetLower: [Character],
        targetOriginal: [Character]
    ) -> SubsequenceScore? {
        var score = 0
        var queryIndex = 0
        var previousMatchIndex = -2
        var firstMatchIndex = -1

        for targetIndex in 0 ..< targetLower.count {
            if queryIndex >= queryLower.count { break }
            if targetLower[targetIndex] != queryLower[queryIndex] { continue }

            if firstMatchIndex < 0 { firstMatchIndex = targetIndex }

            var charScore = 10
            if targetIndex == previousMatchIndex + 1 {
                charScore += 40
            }
            if targetIndex == 0 {
                charScore += 50
            } else {
                let previousChar = targetLower[targetIndex - 1]
                if isSeparator(previousChar) {
                    charScore += 35
                }
                let originalChar = targetOriginal[targetIndex]
                if originalChar.isUppercase, !previousChar.isUppercase {
                    charScore += 25
                }
            }

            score += charScore
            previousMatchIndex = targetIndex
            queryIndex += 1
        }

        guard queryIndex == queryLower.count else { return nil }

        if firstMatchIndex >= 0 {
            score -= firstMatchIndex
        }
        return SubsequenceScore(score: score, firstMatchIndex: max(firstMatchIndex, 0))
    }

    private static func fileNamePositionBonus(nameScore: SubsequenceScore, fileNameLength: Int) -> Int {
        guard fileNameLength > 0 else { return 0 }
        let distanceFromStart = nameScore.firstMatchIndex
        return max(0, 60 - distanceFromStart * 5)
    }

    private static func isSeparator(_ character: Character) -> Bool {
        character == "_" || character == "-" || character == "." || character == "/" || character == " "
    }
}

private final class ResultsBox: @unchecked Sendable {
    private let lock = NSLock()
    private var buffer = ""
    private var results: [FileSearchResult] = []

    func append(chunk: String, projectPath: String, limit: Int) -> Bool {
        lock.lock()
        defer { lock.unlock() }

        if results.count >= limit { return true }

        buffer.append(chunk)

        while let newlineRange = buffer.range(of: "\n") {
            let line = String(buffer[buffer.startIndex ..< newlineRange.lowerBound])
            buffer.removeSubrange(buffer.startIndex ..< newlineRange.upperBound)

            if let result = makeResult(absolutePath: line, projectPath: projectPath) {
                results.append(result)
                if results.count >= limit {
                    return true
                }
            }
        }

        return results.count >= limit
    }

    func take() -> [FileSearchResult] {
        lock.lock()
        defer { lock.unlock() }
        return results
    }

    private func makeResult(absolutePath: String, projectPath: String) -> FileSearchResult? {
        guard !absolutePath.isEmpty else { return nil }
        let url = URL(fileURLWithPath: absolutePath)
        let fileName = url.lastPathComponent
        let prefix = projectPath.hasSuffix("/") ? projectPath : projectPath + "/"
        let relative = absolutePath.hasPrefix(prefix)
            ? String(absolutePath.dropFirst(prefix.count))
            : absolutePath
        return FileSearchResult(
            id: absolutePath,
            relativePath: relative,
            absolutePath: absolutePath,
            fileName: fileName
        )
    }
}
