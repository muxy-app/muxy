import Foundation

struct GitWorktreeRecord: Hashable {
    let path: String
    let branch: String?
    let head: String?
    let isBare: Bool
    let isDetached: Bool
    let isPrunable: Bool

    init(
        path: String,
        branch: String?,
        head: String?,
        isBare: Bool,
        isDetached: Bool,
        isPrunable: Bool = false
    ) {
        self.path = path
        self.branch = branch
        self.head = head
        self.isBare = isBare
        self.isDetached = isDetached
        self.isPrunable = isPrunable
    }
}

protocol GitWorktreeListing {
    func listWorktrees(repoPath: String) async throws -> [GitWorktreeRecord]
}

actor GitWorktreeService: GitWorktreeListing {
    struct WorktreePathResolution {
        let path: String
        let remoteHomePath: String?
    }

    typealias RemovalRunner = @Sendable (
        _ repoPath: String,
        _ arguments: [String],
        _ context: WorkspaceContext,
        _ timeout: TimeInterval
    ) async throws -> GitProcessResult

    static let shared = GitWorktreeService()
    static let defaultWorktreeRemovalTimeout: TimeInterval = 300
    private static let removalReconciliationTimeout: TimeInterval = 5
    private static let maxConcurrentRepositoryChecksPerContext = 4
    private static let localRepositoryCheckTimeout = Duration.seconds(10)

    private let repositoryCheckCoordinator: GitRepositoryCheckCoordinator

    enum GitWorktreeError: LocalizedError {
        case notGitRepository
        case commandFailed(String)

        var errorDescription: String? {
            switch self {
            case .notGitRepository:
                "This folder is not a Git repository."
            case let .commandFailed(message):
                message
            }
        }
    }

    private init() {
        repositoryCheckCoordinator = GitRepositoryCheckCoordinator(
            maxConcurrentChecksPerContext: Self.maxConcurrentRepositoryChecksPerContext
        ) { path, context in
            await GitWorktreeService.probeGitRepository(path: path, context: context)
        }
    }

    func isGitRepository(_ path: String, context: WorkspaceContext = .local) async -> Bool {
        await repositoryCheckCoordinator.isGitRepository(path, context: context)
    }

    private static func probeGitRepository(path: String, context: WorkspaceContext) async -> Bool {
        guard case .local = context else {
            return await runRepositoryProbe(path: path, context: context)
        }
        return await withTaskGroup(of: Bool.self, returning: Bool.self) { group in
            group.addTask { await runRepositoryProbe(path: path, context: .local) }
            group.addTask {
                try? await Task.sleep(for: localRepositoryCheckTimeout)
                return false
            }
            let result = await group.next() ?? false
            group.cancelAll()
            return result
        }
    }

    private static func runRepositoryProbe(path: String, context: WorkspaceContext) async -> Bool {
        guard let result = try? await GitProcessRunner.runGit(
            repoPath: path,
            arguments: ["rev-parse", "--is-inside-work-tree"],
            context: context
        )
        else {
            return false
        }
        return result.status == 0 &&
            result.stdout.trimmingCharacters(in: .whitespacesAndNewlines) == "true"
    }

    func hasUncommittedChanges(worktreePath: String, context: WorkspaceContext = .local) async -> Bool {
        await (try? uncommittedChanges(worktreePath: worktreePath, context: context)) ?? false
    }

    func uncommittedChanges(
        worktreePath: String,
        context: WorkspaceContext = .local,
        timeout: TimeInterval? = nil
    ) async throws -> Bool {
        let result = try await GitProcessRunner.runGit(
            repoPath: worktreePath,
            arguments: ["status", "--porcelain=1", "--untracked-files=all"],
            context: context,
            timeout: timeout
        )
        guard result.status == 0 else {
            throw GitWorktreeError.commandFailed(
                result.stderr.isEmpty ? "Failed to inspect worktree changes." : result.stderr
            )
        }
        return !result.stdout.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    func listWorktrees(repoPath: String) async throws -> [GitWorktreeRecord] {
        try await listWorktrees(repoPath: repoPath, context: .local)
    }

    func listWorktrees(repoPath: String, context: WorkspaceContext) async throws -> [GitWorktreeRecord] {
        let result = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["worktree", "list", "--porcelain"],
            context: context
        )
        guard result.status == 0 else {
            throw GitWorktreeError.commandFailed(
                result.stderr.isEmpty ? "Failed to list worktrees." : result.stderr
            )
        }
        return parsePorcelain(result.stdout)
    }

    static let allowedBranchCharacters = CharacterSet.alphanumerics
        .union(CharacterSet(charactersIn: "._/-"))

    private static func validateBranchName(_ branch: String) throws {
        guard !branch.isEmpty,
              !branch.hasPrefix("-"),
              branch.unicodeScalars.allSatisfy({ Self.allowedBranchCharacters.contains($0) })
        else {
            throw GitWorktreeError.commandFailed("Invalid branch name.")
        }
    }

    func addWorktree(
        repoPath: String,
        path: String,
        branch: String,
        createBranch: Bool,
        baseBranch: String? = nil,
        context: WorkspaceContext = .local
    ) async throws {
        try Self.validateBranchName(branch)
        var args: [String] = ["worktree", "add"]
        if createBranch {
            args += ["-b", branch, path]
            if let baseBranch {
                try Self.validateBranchName(baseBranch)
                args.append(baseBranch)
            }
        } else {
            args += ["--", path, branch]
        }
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: args, context: context)
        guard result.status == 0 else {
            throw GitWorktreeError.commandFailed(
                result.stderr.isEmpty ? "Failed to add worktree." : result.stderr
            )
        }
    }

    func removeWorktree(
        repoPath: String,
        path: String,
        force: Bool = false,
        context: WorkspaceContext = .local,
        timeout: TimeInterval = defaultWorktreeRemovalTimeout,
        removalRunner: RemovalRunner = runRemoval
    ) async throws {
        let deadline = OperationDeadline(timeout: timeout)
        let target = Self.canonicalPath(path, context: context)
        let wasRegistered = try await listWorktrees(
            repoPath: repoPath,
            context: context,
            timeout: deadline.remaining()
        )
        .contains { Self.canonicalPath($0.path, context: context) == target }
        if !wasRegistered, try await context.fileOps.exists(at: path, timeout: deadline.remaining()) {
            throw GitWorktreeError.commandFailed("Worktree is not registered with this repository.")
        }

        var args: [String] = ["worktree", "remove"]
        if force {
            args.append("--force")
        }
        args += ["--", path]
        let result: GitProcessResult
        do {
            result = try await removalRunner(repoPath, args, context, deadline.remaining())
        } catch {
            guard Self.isTimeout(error), try await !isRegistered(
                target: target,
                repoPath: repoPath,
                context: context,
                timeout: Self.removalReconciliationTimeout
            )
            else { throw error }
            return
        }
        guard result.status != 0 else { return }
        if try await context.fileOps.exists(at: path, timeout: deadline.remaining()) {
            throw GitWorktreeError.commandFailed(
                result.stderr.isEmpty ? "Failed to remove worktree." : result.stderr
            )
        }

        if let remaining = try? deadline.remaining() {
            try? await pruneWorktrees(repoPath: repoPath, context: context, timeout: remaining)
        }
        try Task.checkCancellation()
        let verificationTimeout = (try? deadline.remaining()) ?? Self.removalReconciliationTimeout
        let stillRegistered = try await isRegistered(
            target: target,
            repoPath: repoPath,
            context: context,
            timeout: verificationTimeout
        )
        guard stillRegistered else { return }

        throw GitWorktreeError.commandFailed(
            result.stderr.isEmpty ? "Failed to remove worktree." : result.stderr
        )
    }

    private func pruneWorktrees(repoPath: String, context: WorkspaceContext, timeout: TimeInterval) async throws {
        let result = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["worktree", "prune"],
            context: context,
            timeout: timeout
        )
        guard result.status == 0 else {
            throw GitWorktreeError.commandFailed(
                result.stderr.isEmpty ? "Failed to prune worktrees." : result.stderr
            )
        }
    }

    private func isRegistered(
        target: String,
        repoPath: String,
        context: WorkspaceContext,
        timeout: TimeInterval
    ) async throws -> Bool {
        try await listWorktrees(repoPath: repoPath, context: context, timeout: timeout)
            .contains { Self.canonicalPath($0.path, context: context) == target }
    }

    private static func runRemoval(
        repoPath: String,
        arguments: [String],
        context: WorkspaceContext,
        timeout: TimeInterval
    ) async throws -> GitProcessResult {
        try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: arguments,
            context: context,
            timeout: timeout
        )
    }

    private static func isTimeout(_ error: Error) -> Bool {
        if let error = error as? GitProcessError, case .timedOut = error {
            return true
        }
        if let error = error as? SSHCommandError, case .timedOut = error {
            return true
        }
        return false
    }

    private func listWorktrees(
        repoPath: String,
        context: WorkspaceContext,
        timeout: TimeInterval
    ) async throws -> [GitWorktreeRecord] {
        let result = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["worktree", "list", "--porcelain"],
            context: context,
            timeout: timeout
        )
        guard result.status == 0 else {
            throw GitWorktreeError.commandFailed(
                result.stderr.isEmpty ? "Failed to list worktrees." : result.stderr
            )
        }
        return parsePorcelain(result.stdout)
    }

    static func canonicalPath(_ path: String, context: WorkspaceContext = .local) -> String {
        guard !context.isRemote else { return ProjectPickerPathService.standardizedRemotePath(path) }
        let standardized = URL(fileURLWithPath: path).standardizedFileURL
        let resolved = standardized.resolvingSymlinksInPath()
        guard resolved.path == standardized.path else { return resolved.path }

        let parent = standardized.deletingLastPathComponent().resolvingSymlinksInPath()
        return parent.appendingPathComponent(standardized.lastPathComponent).path
    }

    static func resolveWorktreePath(
        _ path: String,
        repoPath: String,
        context: WorkspaceContext,
        timeout: TimeInterval
    ) async throws -> WorktreePathResolution {
        guard case let .ssh(destination) = context else {
            return WorktreePathResolution(
                path: canonicalPath(NSString(string: path).expandingTildeInPath),
                remoteHomePath: nil
            )
        }
        let deadline = OperationDeadline(timeout: timeout)
        let homeResult = try await SSHCommandRunner.run(
            destination: destination,
            remoteCommand: "printf '%s' \"$HOME\"",
            timeout: deadline.remaining()
        )
        guard homeResult.status == 0 else {
            throw GitWorktreeError.commandFailed(
                homeResult.stderr.isEmpty ? "Failed to resolve the remote home directory." : homeResult.stderr
            )
        }
        let homePath = homeResult.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
        let absolutePath = expandedRemotePath(path, repoPath: repoPath, homePath: homePath)
        let quotedPath = RemoteCommandBuilder.quoteRemotePath(absolutePath)
        let resolved = try await SSHCommandRunner.run(
            destination: destination,
            remoteCommand: "if [ -d \(quotedPath) ]; then cd \(quotedPath) && pwd -P; else printf '%s' \(quotedPath); fi",
            timeout: deadline.remaining()
        )
        guard resolved.status == 0 else {
            throw GitWorktreeError.commandFailed(
                resolved.stderr.isEmpty ? "Failed to resolve the remote worktree path." : resolved.stderr
            )
        }
        return WorktreePathResolution(
            path: canonicalPath(
                resolved.stdout.trimmingCharacters(in: .whitespacesAndNewlines),
                context: context
            ),
            remoteHomePath: homePath
        )
    }

    static func expandedRemotePath(_ path: String, repoPath: String, homePath: String) -> String {
        func expandHome(_ value: String) -> String {
            if value == "~" {
                return homePath
            }
            if value.hasPrefix("~/") {
                return homePath + value.dropFirst()
            }
            return value
        }

        let expandedPath = expandHome(path)
        guard !expandedPath.hasPrefix("/") else {
            return ProjectPickerPathService.standardizedRemotePath(expandedPath)
        }
        let expandedRepoPath = expandHome(repoPath)
        return ProjectPickerPathService.standardizedRemotePath(expandedRepoPath + "/" + expandedPath)
    }

    func resolvedRepositoryRoot(repoPath: String, context: WorkspaceContext) async -> String? {
        guard let result = try? await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["rev-parse", "--show-toplevel"],
            context: context
        ), result.status == 0
        else {
            return nil
        }
        let toplevel = result.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
        return toplevel.isEmpty ? nil : toplevel
    }

    private func parsePorcelain(_ raw: String) -> [GitWorktreeRecord] {
        var records: [GitWorktreeRecord] = []
        var currentPath: String?
        var currentBranch: String?
        var currentHead: String?
        var isBare = false
        var isDetached = false
        var isPrunable = false

        func flush() {
            guard let path = currentPath else { return }
            records.append(GitWorktreeRecord(
                path: path,
                branch: currentBranch,
                head: currentHead,
                isBare: isBare,
                isDetached: isDetached,
                isPrunable: isPrunable
            ))
            currentPath = nil
            currentBranch = nil
            currentHead = nil
            isBare = false
            isDetached = false
            isPrunable = false
        }

        for line in raw.split(separator: "\n", omittingEmptySubsequences: false) {
            let trimmed = String(line)
            if trimmed.isEmpty {
                flush()
                continue
            }
            if trimmed.hasPrefix("worktree ") {
                currentPath = String(trimmed.dropFirst("worktree ".count))
            } else if trimmed.hasPrefix("HEAD ") {
                currentHead = String(trimmed.dropFirst("HEAD ".count))
            } else if trimmed.hasPrefix("branch ") {
                let full = String(trimmed.dropFirst("branch ".count))
                currentBranch = full.hasPrefix("refs/heads/")
                    ? String(full.dropFirst("refs/heads/".count))
                    : full
            } else if trimmed == "bare" {
                isBare = true
            } else if trimmed == "detached" {
                isDetached = true
            } else if trimmed == "prunable" || trimmed.hasPrefix("prunable ") {
                isPrunable = true
            }
        }
        flush()
        return records
    }
}
