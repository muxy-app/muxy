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

    typealias EnvironmentProvider = WorktreeHookEnvironment.Provider

    static func run(
        sourceProjectPath: String,
        worktree: Worktree,
        projectHookApproval: WorktreeConfig.ProjectHookApproval? = nil,
        timeout: TimeInterval = defaultTimeout,
        globalConfigURL: URL = WorktreeConfig.globalConfigURL(),
        environmentProvider: EnvironmentProvider = { sourceProjectPath, worktree, timeout in
            try await WorktreeHookEnvironment.hydratedValues(
                sourceProjectPath: sourceProjectPath,
                worktree: worktree,
                timeout: timeout
            )
        },
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

        let deadline = OperationDeadline(timeout: timeout)
        let environment: [String: String]
        do {
            environment = try await environmentProvider(sourceProjectPath, worktree, deadline.remaining())
        } catch {
            logger.error("Could not hydrate setup hook environment: \(error.localizedDescription)")
            return
        }
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
    typealias Provider = @Sendable (
        _ sourceProjectPath: String,
        _ worktree: Worktree,
        _ timeout: TimeInterval
    ) async throws -> [String: String]

    static func hydratedValues(
        sourceProjectPath: String,
        worktree: Worktree,
        timeout: TimeInterval
    ) async throws -> [String: String] {
        try await LoginShellPath.hydrateIfNeeded(timeout: timeout)
        return values(sourceProjectPath: sourceProjectPath, worktree: worktree)
    }

    static func hydratedValues(
        sourceProjectPath: String,
        worktree: Worktree,
        hydrate: @Sendable () async -> Void = { await LoginShellPath.hydrateIfNeeded() },
        baseEnvironment: [String: String] = ProcessInfo.processInfo.environment,
        pathEnvironment: @Sendable () -> String = { LoginShellPath.current }
    ) async -> [String: String] {
        await hydrate()
        return values(
            sourceProjectPath: sourceProjectPath,
            worktree: worktree,
            baseEnvironment: baseEnvironment,
            pathEnvironment: pathEnvironment
        )
    }

    static func values(
        sourceProjectPath: String,
        worktree: Worktree,
        baseEnvironment: [String: String] = ProcessInfo.processInfo.environment,
        pathEnvironment: @Sendable () -> String = { LoginShellPath.current }
    ) -> [String: String] {
        var environment = baseEnvironment
        environment["PATH"] = pathEnvironment()
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
