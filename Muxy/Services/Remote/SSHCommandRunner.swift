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
        timeout: TimeInterval = defaultTimeout
    ) async throws -> GitProcessResult {
        let options = batch ? SSHDestination.batchOptions : SSHDestination.connectOptions
        let arguments = destination.connectionArguments + options + ["-T", destination.target, "--", remoteCommand]
        let resolved = ResolvedLaunch(
            executable: "/usr/bin/ssh",
            arguments: arguments,
            workingDirectory: nil
        )
        return try await withThrowingTaskGroup(of: GitProcessResult.self) { group in
            group.addTask {
                try await GitProcessRunner.runResolved(resolved, lineLimit: lineLimit)
            }
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
