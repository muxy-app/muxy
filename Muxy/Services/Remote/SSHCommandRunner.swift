import Foundation
import MuxySSH

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

    private struct LimitedOutput {
        let stdout: String
        let stdoutData: Data
        let truncated: Bool
    }

    static func run(
        destination: SSHDestination,
        remoteCommand: String,
        batch: Bool = true,
        lineLimit: Int? = nil,
        timeout: TimeInterval = defaultTimeout,
        input: Data? = nil
    ) async throws -> GitProcessResult {
        try SSHImplementationSelection.validate(destination: destination)
        switch SSHImplementationMode.current {
        case .cli:
            let options = batch ? destination.batchOptions : destination.connectOptions
            let command = RemoteCommandBuilder.environmentPrefix(destination.environment) + remoteCommand
            let resolved = ResolvedLaunch(
                executable: "/usr/bin/ssh",
                arguments: destination.connectionArguments + options + ["-T", destination.target, "--", command],
                workingDirectory: nil
            )
            let result = try await withTimeout(timeout) {
                try await GitProcessRunner.runResolved(resolved, lineLimit: lineLimit, stdinData: input)
            }
            let limited = limit(result.stdout, lineLimit: lineLimit)
            return GitProcessResult(
                status: result.status,
                stdout: limited.stdout,
                stdoutData: limited.stdoutData,
                stderr: result.stderr,
                truncated: result.truncated || limited.truncated
            )
        case .native:
            let configuration = SSHConnectionConfiguration.make(destination: destination)
            let result = try await SSHExecService.shared.run(
                configuration: configuration,
                command: RemoteCommandBuilder.environmentPrefix(destination.environment) + remoteCommand,
                stdinData: input,
                timeout: timeout
            )
            let limited = limit(result.stdout, lineLimit: lineLimit)
            return GitProcessResult(
                status: result.status,
                stdout: limited.stdout,
                stdoutData: limited.stdoutData,
                stderr: result.stderr,
                truncated: limited.truncated
            )
        }
    }

    static func runCommand(
        destination: SSHDestination,
        executable: String,
        arguments: [String],
        workingDirectory: String?,
        environment: [String: String]? = nil,
        lineLimit: Int? = nil,
        timeout: TimeInterval = defaultTimeout,
        input: Data? = nil
    ) async throws -> GitProcessResult {
        try SSHImplementationSelection.validate(destination: destination)
        switch SSHImplementationMode.current {
        case .cli:
            let resolved = CommandTransform.resolve(
                executable: executable,
                arguments: arguments,
                workingDirectory: workingDirectory,
                environment: environment,
                in: .ssh(destination)
            )
            return try await withTimeout(timeout) {
                try await GitProcessRunner.runResolved(resolved, lineLimit: lineLimit, stdinData: input)
            }
        case .native:
            let mergedEnvironment = SSHEnvironmentVariables.merged(device: destination.environment, command: environment)
            let remoteCommand = RemoteCommandBuilder.remoteCommand(
                executable: executable,
                arguments: arguments,
                workingDirectory: workingDirectory,
                environment: mergedEnvironment
            )
            return try await run(
                destination: destination,
                remoteCommand: remoteCommand,
                lineLimit: lineLimit,
                timeout: timeout,
                input: input
            )
        }
    }

    static func runShell(
        destination: SSHDestination,
        shellCommand: String,
        workingDirectory: String?,
        environment: [String: String]? = nil,
        lineLimit: Int? = nil,
        timeout: TimeInterval = defaultTimeout,
        input: Data? = nil
    ) async throws -> GitProcessResult {
        try SSHImplementationSelection.validate(destination: destination)
        switch SSHImplementationMode.current {
        case .cli:
            let resolved = CommandTransform.resolveShell(
                shellCommand: shellCommand,
                workingDirectory: workingDirectory,
                environment: environment,
                in: .ssh(destination)
            )
            return try await withTimeout(timeout) {
                try await GitProcessRunner.runResolved(resolved, lineLimit: lineLimit, stdinData: input)
            }
        case .native:
            let mergedEnvironment = SSHEnvironmentVariables.merged(device: destination.environment, command: environment)
            let remoteCommand = RemoteCommandBuilder.remoteShellCommand(
                shell: shellCommand,
                workingDirectory: workingDirectory,
                environment: mergedEnvironment
            )
            return try await run(
                destination: destination,
                remoteCommand: remoteCommand,
                lineLimit: lineLimit,
                timeout: timeout,
                input: input
            )
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

    private static func limit(_ stdout: String, lineLimit: Int?) -> LimitedOutput {
        guard let lineLimit else {
            let data = Data(stdout.utf8)
            return LimitedOutput(stdout: stdout, stdoutData: data, truncated: false)
        }
        var lines = stdout.split(omittingEmptySubsequences: false, whereSeparator: \.isNewline).map(String.init)
        let hadTrailingNewline = stdout.hasSuffix("\n")
        if hadTrailingNewline, lines.last?.isEmpty == true {
            lines.removeLast()
        }
        guard lines.count > lineLimit else {
            let normalized = hadTrailingNewline ? stdout : lines.joined(separator: "\n")
            return LimitedOutput(stdout: normalized, stdoutData: Data(normalized.utf8), truncated: false)
        }
        let limitedLines = Array(lines.prefix(lineLimit))
        let limited = limitedLines.joined(separator: "\n")
        return LimitedOutput(stdout: limited, stdoutData: Data(limited.utf8), truncated: true)
    }
}
