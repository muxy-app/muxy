import Foundation

enum SSHCommandError: LocalizedError {
    case timedOut(TimeInterval)

    var errorDescription: String? {
        switch self {
        case let .timedOut(seconds): "The remote command timed out after \(Int(seconds))s."
        }
    }
}

enum SSHCommandRunner {
    static let defaultTimeout: TimeInterval = 60

    static func run(
        destination: SSHDestination,
        remoteCommand: String,
        batch: Bool = true,
        lineLimit: Int? = nil,
        timeout: TimeInterval = defaultTimeout,
        input: Data? = nil
    ) async throws -> GitProcessResult {
        let options = batch ? SSHDestination.batchOptions : SSHDestination.connectOptions
        let command = RemoteCommandBuilder.environmentPrefix(destination.environment) + remoteCommand
        let arguments = destination.connectionArguments + options + ["-T", destination.target, "--", command]
        let resolved = ResolvedLaunch(
            executable: "/usr/bin/ssh",
            arguments: arguments,
            workingDirectory: nil
        )
        return try await withTimeout(timeout) {
            try await GitProcessRunner.runResolved(resolved, lineLimit: lineLimit, stdinData: input)
        }
    }

    static func withTimeout(
        _ timeout: TimeInterval,
        operation: @escaping @Sendable () async throws -> GitProcessResult
    ) async throws -> GitProcessResult {
        try await withThrowingTaskGroup(of: GitProcessResult.self) { group in
            group.addTask { try await operation() }
            group.addTask {
                try await Task.sleep(nanoseconds: UInt64(timeout * 1_000_000_000))
                throw SSHCommandError.timedOut(timeout)
            }
            defer { group.cancelAll() }
            guard let result = try await group.next() else {
                throw SSHCommandError.timedOut(timeout)
            }
            return result
        }
    }
}
