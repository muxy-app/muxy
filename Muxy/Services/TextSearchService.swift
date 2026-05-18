import Foundation

struct TextSearchMatch: Identifiable, Equatable {
    let id: String
    let absolutePath: String
    let relativePath: String
    let lineNumber: Int
    let column: Int
    let lineText: String
    let matchByteStart: Int
    let matchByteLength: Int
}

struct TextSearchOptions: Equatable {
    var caseSensitive: Bool = false
    var wholeWord: Bool = false
}

enum TextSearchService {
    static let maxResults = 200
    static let maxResultsPerFile = 20
    static let minQueryLength = 2

    static func search(
        query: String,
        in projectPath: String,
        options: TextSearchOptions = TextSearchOptions(),
        coordinator: SearchCoordinator? = nil
    ) async -> [TextSearchMatch] {
        let trimmed = query.trimmingCharacters(in: .whitespaces)
        guard trimmed.count >= minQueryLength else { return [] }
        guard let executable = ripgrepExecutableURL() else { return [] }
        guard let patternData = trimmed.data(using: .utf8) else { return [] }

        let runner = coordinator ?? SearchCoordinator()
        return await runner.run(
            executable: executable,
            patternData: patternData,
            patternByteLength: patternData.count,
            projectPath: projectPath,
            options: options
        )
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

    static func arguments(projectPath: String, options: TextSearchOptions) -> [String] {
        var args: [String] = [
            "--vimgrep",
            "--max-count", "\(maxResultsPerFile)",
            "--max-columns", "300",
            "--max-filesize", "2M",
            "--no-config",
            "-F",
        ]
        args.append(options.caseSensitive ? "--case-sensitive" : "--smart-case")
        if options.wholeWord { args.append("--word-regexp") }
        args.append(contentsOf: ["-f", "-", "--", projectPath])
        return args
    }

    static func parseVimgrepLine(
        _ line: Substring,
        projectPath: String,
        patternByteLength: Int
    ) -> TextSearchMatch? {
        guard let firstColon = line.firstIndex(of: ":") else { return nil }
        let absolutePath = String(line[..<firstColon])
        let afterPath = line.index(after: firstColon)

        guard let secondColon = line[afterPath...].firstIndex(of: ":") else { return nil }
        guard let lineNumber = Int(line[afterPath ..< secondColon]) else { return nil }
        let afterLine = line.index(after: secondColon)

        guard let thirdColon = line[afterLine...].firstIndex(of: ":") else { return nil }
        guard let column = Int(line[afterLine ..< thirdColon]) else { return nil }
        let textStart = line.index(after: thirdColon)

        let lineText = String(line[textStart...])

        let prefix = projectPath.hasSuffix("/") ? projectPath : projectPath + "/"
        let relative = absolutePath.hasPrefix(prefix)
            ? String(absolutePath.dropFirst(prefix.count))
            : absolutePath

        let byteStart = max(0, column - 1)
        let byteLength = min(patternByteLength, max(0, lineText.utf8.count - byteStart))

        return TextSearchMatch(
            id: "\(absolutePath):\(lineNumber):\(byteStart)",
            absolutePath: absolutePath,
            relativePath: relative,
            lineNumber: lineNumber,
            column: column,
            lineText: lineText,
            matchByteStart: byteStart,
            matchByteLength: byteLength
        )
    }
}

actor SearchCoordinator {
    private var current: (process: Process, exit: Task<Void, Never>)?

    func run(
        executable: URL,
        patternData: Data,
        patternByteLength: Int,
        projectPath: String,
        options: TextSearchOptions
    ) async -> [TextSearchMatch] {
        if let active = current {
            if active.process.isRunning { active.process.terminate() }
            await active.exit.value
            current = nil
        }

        let process = Process()
        process.executableURL = executable
        process.arguments = TextSearchService.arguments(projectPath: projectPath, options: options)

        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()
        let stdinPipe = Pipe()
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe
        process.standardInput = stdinPipe

        let outputBox = OutputBox()
        let stdoutHandle = stdoutPipe.fileHandleForReading
        stdoutHandle.readabilityHandler = { handle in
            let data = handle.availableData
            if data.isEmpty { return }
            outputBox.append(data)
        }

        let exitTask = Task<Void, Never> {
            await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
                process.terminationHandler = { _ in
                    stdoutHandle.readabilityHandler = nil
                    if let remaining = try? stdoutHandle.readToEnd(), !remaining.isEmpty {
                        outputBox.append(remaining)
                    }
                    continuation.resume()
                }
            }
        }

        do {
            try process.run()
            stdinPipe.fileHandleForWriting.write(patternData)
            try? stdinPipe.fileHandleForWriting.close()
        } catch {
            stdoutHandle.readabilityHandler = nil
            try? stdinPipe.fileHandleForWriting.close()
            return []
        }

        current = (process, exitTask)

        return await withTaskCancellationHandler {
            await exitTask.value
            if current?.process === process { current = nil }
            return outputBox.parseMatches(projectPath: projectPath, patternByteLength: patternByteLength)
        } onCancel: {
            if process.isRunning { process.terminate() }
        }
    }
}

private final class OutputBox: @unchecked Sendable {
    private let lock = NSLock()
    private var buffer = Data()

    func append(_ data: Data) {
        lock.lock()
        defer { lock.unlock() }
        buffer.append(data)
    }

    func parseMatches(projectPath: String, patternByteLength: Int) -> [TextSearchMatch] {
        lock.lock()
        let data = buffer
        lock.unlock()

        guard let text = String(data: data, encoding: .utf8) else { return [] }

        var matches: [TextSearchMatch] = []
        matches.reserveCapacity(TextSearchService.maxResults)

        for line in text.split(separator: "\n", omittingEmptySubsequences: true) {
            guard let match = TextSearchService.parseVimgrepLine(
                line,
                projectPath: projectPath,
                patternByteLength: patternByteLength
            )
            else { continue }
            matches.append(match)
            if matches.count >= TextSearchService.maxResults { break }
        }

        return matches
    }
}
