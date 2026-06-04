import Foundation

struct WorktreeTeardownOutputLine: Hashable, Identifiable {
    enum Channel: Hashable {
        case stdout
        case stderr
        case command
        case status
    }

    let id = UUID()
    let channel: Channel
    let text: String
}

enum WorktreeTeardownError: LocalizedError {
    case commandFailed(command: String)

    var errorDescription: String? {
        switch self {
        case let .commandFailed(command):
            "Teardown command failed: \(command)"
        }
    }
}

enum WorktreeTeardownRunner {
    typealias Executor = @Sendable (
        _ command: String,
        _ worktree: Worktree,
        _ environment: [String: String],
        _ emit: @Sendable @escaping (WorktreeTeardownOutputLine) -> Void
    ) async throws -> Int32

    static func run(
        sourceProjectPath: String,
        worktree: Worktree,
        emit: @Sendable @escaping (WorktreeTeardownOutputLine) -> Void = { _ in },
        executor: Executor = execute
    ) async throws {
        guard !worktree.isExternallyManaged,
              FileManager.default.fileExists(atPath: worktree.path),
              let config = WorktreeConfig.load(fromProjectPath: sourceProjectPath)
        else { return }

        let commands = config.teardown
            .map(\.command)
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
        guard !commands.isEmpty else { return }

        let environment = environment(for: worktree)
        for command in commands {
            emit(WorktreeTeardownOutputLine(channel: .command, text: "$ \(command)"))
            let status = try await executor(command, worktree, environment, emit)
            guard status == 0 else {
                emit(WorktreeTeardownOutputLine(
                    channel: .status,
                    text: "Command exited with status \(status)."
                ))
                throw WorktreeTeardownError.commandFailed(command: command)
            }
        }
    }

    private static func execute(
        command: String,
        worktree: Worktree,
        environment: [String: String],
        emit: @Sendable @escaping (WorktreeTeardownOutputLine) -> Void
    ) async throws -> Int32 {
        try await WorktreeTeardownProcess.run(
            command: command,
            workingDirectory: worktree.path,
            environment: environment,
            emit: emit
        )
    }

    private static func environment(for worktree: Worktree) -> [String: String] {
        var environment = ProcessInfo.processInfo.environment
        environment["MUXY_WORKTREE_PATH"] = worktree.path
        environment["MUXY_WORKTREE_NAME"] = worktree.name
        environment["MUXY_WORKTREE_BRANCH"] = worktree.branch ?? ""
        return environment
    }
}

enum WorktreeTeardownProcess {
    static func run(
        command: String,
        workingDirectory: String,
        environment: [String: String],
        emit: @Sendable @escaping (WorktreeTeardownOutputLine) -> Void
    ) async throws -> Int32 {
        try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                do {
                    let status = try runProcess(
                        command: command,
                        workingDirectory: workingDirectory,
                        environment: environment,
                        emit: emit
                    )
                    continuation.resume(returning: status)
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }

    private static func runProcess(
        command: String,
        workingDirectory: String,
        environment: [String: String],
        emit: @Sendable @escaping (WorktreeTeardownOutputLine) -> Void
    ) throws -> Int32 {
        let process = Process()
        let shell = environment["SHELL"].flatMap { FileManager.default.isExecutableFile(atPath: $0) ? $0 : nil }
            ?? "/bin/zsh"
        process.executableURL = URL(fileURLWithPath: shell)
        process.arguments = ["-c", command]
        process.environment = environment
        process.currentDirectoryURL = URL(fileURLWithPath: workingDirectory)

        let stdout = Pipe()
        let stderr = Pipe()
        process.standardOutput = stdout
        process.standardError = stderr

        let stdoutBuffer = LineBuffer { line in
            emit(WorktreeTeardownOutputLine(channel: .stdout, text: line))
        }
        let stderrBuffer = LineBuffer { line in
            emit(WorktreeTeardownOutputLine(channel: .stderr, text: line))
        }

        stdout.fileHandleForReading.readabilityHandler = { handle in
            let data = handle.availableData
            if data.isEmpty {
                handle.readabilityHandler = nil
                stdoutBuffer.flush()
                return
            }
            stdoutBuffer.append(data)
        }
        stderr.fileHandleForReading.readabilityHandler = { handle in
            let data = handle.availableData
            if data.isEmpty {
                handle.readabilityHandler = nil
                stderrBuffer.flush()
                return
            }
            stderrBuffer.append(data)
        }

        try process.run()
        process.waitUntilExit()

        stdout.fileHandleForReading.readabilityHandler = nil
        stderr.fileHandleForReading.readabilityHandler = nil
        stdoutBuffer.append(stdout.fileHandleForReading.readDataToEndOfFile())
        stderrBuffer.append(stderr.fileHandleForReading.readDataToEndOfFile())
        stdoutBuffer.flush()
        stderrBuffer.flush()

        return process.terminationStatus
    }
}

private final class LineBuffer: @unchecked Sendable {
    private let queue = DispatchQueue(label: "app.muxy.teardown-line-buffer")
    private var pending = Data()
    private let onLine: (String) -> Void

    init(onLine: @escaping (String) -> Void) {
        self.onLine = onLine
    }

    func append(_ data: Data) {
        queue.sync {
            pending.append(data)
            while let newlineRange = pending.range(of: Data([0x0A])) {
                let lineData = pending.subdata(in: 0 ..< newlineRange.lowerBound)
                pending.removeSubrange(0 ..< newlineRange.upperBound)
                emit(lineData)
            }
        }
    }

    func flush() {
        queue.sync {
            guard !pending.isEmpty else { return }
            let lineData = pending
            pending.removeAll(keepingCapacity: false)
            emit(lineData)
        }
    }

    private func emit(_ data: Data) {
        let text = String(data: data, encoding: .utf8) ?? ""
        let trimmed = text.trimmingCharacters(in: CharacterSet(charactersIn: "\r"))
        guard !trimmed.isEmpty else { return }
        onLine(trimmed)
    }
}
