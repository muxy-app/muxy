import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "WorktreeSetupRunner")

enum WorktreeSetupRunner {
    static let defaultTimeout: TimeInterval = 300

    typealias Executor = @Sendable (
        _ command: String,
        _ worktree: Worktree,
        _ environment: [String: String],
        _ timeout: TimeInterval
    ) async throws -> Int32

    static func run(
        sourceProjectPath: String,
        worktree: Worktree,
        projectHookApproval: WorktreeConfig.ProjectHookApproval? = nil,
        timeout: TimeInterval = defaultTimeout,
        globalConfigURL: URL = WorktreeConfig.globalConfigURL(),
        executor: Executor = execute
    ) async {
        guard FileManager.default.fileExists(atPath: worktree.path) else { return }
        let commands: [String]
        do {
            commands = try WorktreeConfig.commandsForExecution(
                WorktreeConfig.resolvedSetupCommands(
                    sourceProjectPath: sourceProjectPath,
                    globalConfigURL: globalConfigURL,
                    includeProjectCommands: projectHookApproval != nil
                ),
                projectHookApproval: projectHookApproval
            )
        } catch {
            logger.error("Could not load setup hooks: \(error.localizedDescription)")
            return
        }
        guard !commands.isEmpty else { return }

        let environment = WorktreeHookEnvironment.values(
            sourceProjectPath: sourceProjectPath,
            worktree: worktree
        )
        let deadline = OperationDeadline(timeout: timeout)
        for command in commands {
            do {
                let status = try await executor(command, worktree, environment, deadline.remaining())
                guard status == 0 else {
                    logger.error("Setup command failed for worktree \(worktree.id) with status \(status)")
                    return
                }
            } catch {
                logger.error("Setup command failed for worktree \(worktree.id): \(error.localizedDescription)")
                return
            }
        }
    }

    private static func execute(
        command: String,
        worktree: Worktree,
        environment: [String: String],
        timeout: TimeInterval
    ) async throws -> Int32 {
        try await WorktreeHookProcess.run(
            command: command,
            workingDirectory: worktree.path,
            environment: environment,
            timeout: timeout
        )
    }
}

enum WorktreeHookEnvironment {
    static func values(sourceProjectPath: String, worktree: Worktree) -> [String: String] {
        var environment = ProcessInfo.processInfo.environment
        environment["MUXY_PROJECT_PATH"] = sourceProjectPath
        environment["MUXY_WORKTREE_ID"] = worktree.id.uuidString
        environment["MUXY_WORKTREE_PATH"] = worktree.path
        environment["MUXY_WORKTREE_NAME"] = worktree.name
        environment["MUXY_WORKTREE_BRANCH"] = worktree.branch ?? ""
        return environment
    }
}

enum WorktreeHookProcess {
    static func run(
        command: String,
        workingDirectory: String,
        environment: [String: String],
        timeout: TimeInterval,
        onStandardOutput: @Sendable @escaping (Data) -> Void = { _ in },
        onStandardError: @Sendable @escaping (Data) -> Void = { _ in }
    ) async throws -> Int32 {
        let shell = environment["SHELL"].flatMap { FileManager.default.isExecutableFile(atPath: $0) ? $0 : nil }
            ?? "/bin/zsh"
        let result = try await SubprocessRunner.run(SubprocessRequest(
            executablePath: shell,
            arguments: ["-c", command],
            workingDirectory: workingDirectory,
            environment: environment,
            timeout: timeout,
            onStandardOutput: onStandardOutput,
            onStandardError: onStandardError
        ))
        return result.status
    }
}
