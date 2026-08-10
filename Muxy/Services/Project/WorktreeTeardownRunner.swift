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
    static let defaultTimeout: TimeInterval = 300

    typealias Executor = @Sendable (
        _ command: String,
        _ worktree: Worktree,
        _ environment: [String: String],
        _ timeout: TimeInterval,
        _ emit: @Sendable @escaping (WorktreeTeardownOutputLine) -> Void
    ) async throws -> Int32

    static func run(
        sourceProjectPath: String,
        worktree: Worktree,
        timeout: TimeInterval = defaultTimeout,
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
        let deadline = OperationDeadline(timeout: timeout)
        for command in commands {
            emit(WorktreeTeardownOutputLine(channel: .command, text: "$ \(command)"))
            let status = try await executor(command, worktree, environment, deadline.remaining(), emit)
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
        timeout: TimeInterval,
        emit: @Sendable @escaping (WorktreeTeardownOutputLine) -> Void
    ) async throws -> Int32 {
        try await WorktreeTeardownProcess.run(
            command: command,
            workingDirectory: worktree.path,
            environment: environment,
            timeout: timeout,
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
        timeout: TimeInterval = WorktreeTeardownRunner.defaultTimeout,
        emit: @Sendable @escaping (WorktreeTeardownOutputLine) -> Void
    ) async throws -> Int32 {
        let shell = environment["SHELL"].flatMap { FileManager.default.isExecutableFile(atPath: $0) ? $0 : nil }
            ?? "/bin/zsh"
        let stdoutBuffer = LineBuffer { line in
            emit(WorktreeTeardownOutputLine(channel: .stdout, text: line))
        }
        let stderrBuffer = LineBuffer { line in
            emit(WorktreeTeardownOutputLine(channel: .stderr, text: line))
        }
        defer {
            stdoutBuffer.flush()
            stderrBuffer.flush()
        }
        let result = try await SubprocessRunner.run(SubprocessRequest(
            executablePath: shell,
            arguments: ["-c", command],
            workingDirectory: workingDirectory,
            environment: environment,
            timeout: timeout,
            onStandardOutput: { stdoutBuffer.append($0) },
            onStandardError: { stderrBuffer.append($0) }
        ))
        return result.status
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
