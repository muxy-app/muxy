import Foundation

struct TextSearchMatch: Identifiable, Equatable {
    let id: String
    let absolutePath: String
    let relativePath: String
    let lineNumber: Int
    let column: Int
    let lineText: String
    let matchStart: Int
    let matchEnd: Int
}

enum TextSearchService {
    static let maxResults = 200
    static let minQueryLength = 2

    static func search(query: String, in projectPath: String) async -> [TextSearchMatch] {
        let trimmed = query.trimmingCharacters(in: .whitespaces)
        guard trimmed.count >= minQueryLength else { return [] }
        guard let executable = ripgrepExecutableURL() else { return [] }
        guard let patternData = trimmed.data(using: .utf8) else { return [] }

        return await withCheckedContinuation { continuation in
            let process = Process()
            process.executableURL = executable
            process.arguments = arguments(projectPath: projectPath)

            let stdoutPipe = Pipe()
            let stderrPipe = Pipe()
            let stdinPipe = Pipe()
            process.standardOutput = stdoutPipe
            process.standardError = stderrPipe
            process.standardInput = stdinPipe

            let resultsBox = MatchesBox()
            let handle = stdoutPipe.fileHandleForReading

            handle.readabilityHandler = { fileHandle in
                let data = fileHandle.availableData
                if data.isEmpty { return }
                guard let chunk = String(data: data, encoding: .utf8) else { return }
                let done = resultsBox.append(chunk: chunk, projectPath: projectPath, limit: maxResults)
                if done, process.isRunning {
                    process.terminate()
                }
            }

            process.terminationHandler = { _ in
                handle.readabilityHandler = nil
                if let remaining = try? handle.readToEnd(), let chunk = String(data: remaining, encoding: .utf8) {
                    _ = resultsBox.append(chunk: chunk, projectPath: projectPath, limit: maxResults)
                }
                continuation.resume(returning: resultsBox.take())
            }

            do {
                try process.run()
                stdinPipe.fileHandleForWriting.write(patternData)
                try? stdinPipe.fileHandleForWriting.close()
            } catch {
                handle.readabilityHandler = nil
                try? stdinPipe.fileHandleForWriting.close()
                continuation.resume(returning: [])
            }
        }
    }

    static func ripgrepExecutableURL() -> URL? {
        if let bundled = Bundle.main.url(forResource: "rg", withExtension: nil) {
            return bundled
        }
        let pathCandidates = ["/opt/homebrew/bin/rg", "/usr/local/bin/rg", "/usr/bin/rg"]
        for candidate in pathCandidates where FileManager.default.isExecutableFile(atPath: candidate) {
            return URL(fileURLWithPath: candidate)
        }
        return nil
    }

    private static func arguments(projectPath: String) -> [String] {
        [
            "--json",
            "--smart-case",
            "--max-count", "20",
            "--max-columns", "300",
            "--max-filesize", "2M",
            "--no-config",
            "-f",
            "-",
            "--",
            projectPath,
        ]
    }

    static func parseLine(_ line: String, projectPath: String) -> TextSearchMatch? {
        guard let data = line.data(using: .utf8) else { return nil }
        guard let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        guard let type = object["type"] as? String, type == "match" else { return nil }
        guard let payload = object["data"] as? [String: Any] else { return nil }
        guard let pathDict = payload["path"] as? [String: Any] else { return nil }
        guard let absolutePath = pathDict["text"] as? String else { return nil }
        guard let lineNumber = payload["line_number"] as? Int else { return nil }
        guard let lineDict = payload["lines"] as? [String: Any] else { return nil }
        guard var lineText = lineDict["text"] as? String else { return nil }
        if lineText.hasSuffix("\n") { lineText.removeLast() }
        if lineText.hasSuffix("\r") { lineText.removeLast() }

        var matchStart = 0
        var matchEnd = lineText.utf8.count
        if let submatches = payload["submatches"] as? [[String: Any]], let first = submatches.first {
            matchStart = (first["start"] as? Int) ?? 0
            matchEnd = (first["end"] as? Int) ?? matchStart
        }

        let prefix = projectPath.hasSuffix("/") ? projectPath : projectPath + "/"
        let relative = absolutePath.hasPrefix(prefix)
            ? String(absolutePath.dropFirst(prefix.count))
            : absolutePath

        let column = columnFromUTF8Offset(matchStart, in: lineText)

        return TextSearchMatch(
            id: "\(absolutePath):\(lineNumber):\(matchStart)",
            absolutePath: absolutePath,
            relativePath: relative,
            lineNumber: lineNumber,
            column: column,
            lineText: lineText,
            matchStart: matchStart,
            matchEnd: matchEnd
        )
    }

    static func columnFromUTF8Offset(_ utf8Offset: Int, in text: String) -> Int {
        guard utf8Offset > 0 else { return 1 }
        let utf8 = text.utf8
        var consumed = 0
        var characterCount = 0
        var index = utf8.startIndex
        while index != utf8.endIndex, consumed < utf8Offset {
            consumed += 1
            index = utf8.index(after: index)
            if let scalarIndex = index.samePosition(in: text.unicodeScalars) {
                characterCount = text.unicodeScalars.distance(from: text.unicodeScalars.startIndex, to: scalarIndex)
            }
        }
        return characterCount + 1
    }
}

private final class MatchesBox: @unchecked Sendable {
    private let lock = NSLock()
    private var buffer = ""
    private var matches: [TextSearchMatch] = []

    func append(chunk: String, projectPath: String, limit: Int) -> Bool {
        lock.lock()
        defer { lock.unlock() }

        if matches.count >= limit { return true }

        buffer.append(chunk)

        while let newlineRange = buffer.range(of: "\n") {
            let line = String(buffer[buffer.startIndex ..< newlineRange.lowerBound])
            buffer.removeSubrange(buffer.startIndex ..< newlineRange.upperBound)

            if line.isEmpty { continue }
            if let match = TextSearchService.parseLine(line, projectPath: projectPath) {
                matches.append(match)
                if matches.count >= limit { return true }
            }
        }

        return matches.count >= limit
    }

    func take() -> [TextSearchMatch] {
        lock.lock()
        defer { lock.unlock() }
        return matches
    }
}
