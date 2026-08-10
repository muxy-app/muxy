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
    struct WorktreePathResolution: Sendable {
        let path: String
        let identityPaths: Set<String>
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
    private static let maximumCanonicalPathOutputByteCount = 1024 * 1024
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

    func uncommittedChangesIfRegistered(
        repoPath: String,
        resolution: WorktreePathResolution,
        context: WorkspaceContext,
        deadline: OperationDeadline
    ) async throws -> Bool {
        guard try await isWorktreeRegistered(
            repoPath: repoPath,
            resolution: resolution,
            context: context,
            deadline: deadline
        )
        else { return false }

        let hasChanges: Bool
        do {
            hasChanges = try await uncommittedChanges(
                worktreePath: resolution.path,
                context: context,
                timeout: deadline.remaining()
            )
        } catch {
            guard try await isWorktreeRegistered(
                repoPath: repoPath,
                resolution: resolution,
                context: context,
                deadline: deadline
            )
            else { return false }
            throw error
        }
        guard hasChanges else { return false }
        return try await isWorktreeRegistered(
            repoPath: repoPath,
            resolution: resolution,
            context: context,
            deadline: deadline
        )
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

    @discardableResult
    func removeWorktree(
        repoPath: String,
        path: String,
        force: Bool = false,
        context: WorkspaceContext = .local,
        timeout: TimeInterval = defaultWorktreeRemovalTimeout,
        trustedTrackedPath: Bool = false,
        removalRunner: RemovalRunner = runRemoval
    ) async throws -> WorktreePathResolution {
        let deadline = OperationDeadline(timeout: timeout)
        let resolution = try await Self.resolveWorktreePath(
            path,
            repoPath: repoPath,
            context: context,
            deadline: deadline
        )
        return try await removeWorktree(
            repoPath: repoPath,
            resolution: resolution,
            force: force,
            context: context,
            deadline: deadline,
            trustedTrackedPath: trustedTrackedPath,
            removalRunner: removalRunner
        )
    }

    @discardableResult
    func removeWorktree(
        repoPath: String,
        resolution: WorktreePathResolution,
        force: Bool = false,
        context: WorkspaceContext = .local,
        deadline: OperationDeadline,
        trustedTrackedPath: Bool = false,
        removalRunner: RemovalRunner = runRemoval
    ) async throws -> WorktreePathResolution {
        let path = resolution.path
        let wasRegistered = try await isRegistered(
            resolution: resolution,
            repoPath: repoPath,
            context: context,
            timeout: deadline.remaining()
        )
        if !wasRegistered {
            if trustedTrackedPath {
                return resolution
            }
            if try await context.fileOps.exists(at: path, timeout: deadline.remaining()) {
                throw GitWorktreeError.commandFailed("Worktree is not registered with this repository.")
            }
            return resolution
        }

        var args: [String] = ["worktree", "remove"]
        if force {
            args.append("--force")
        }
        args += ["--", path]
        let result: GitProcessResult
        do {
            result = try await removalRunner(repoPath, args, context, deadline.remaining())
        } catch let removalError {
            guard Self.isTimeout(removalError) else { throw removalError }
            try Task.checkCancellation()
            let remainsRegistered: Bool
            do {
                remainsRegistered = try await isRegistered(
                    resolution: resolution,
                    repoPath: repoPath,
                    context: context,
                    timeout: deadline.remaining(upTo: Self.removalReconciliationTimeout)
                )
            } catch {
                try Task.checkCancellation()
                throw removalError
            }
            guard !remainsRegistered else { throw removalError }
            return resolution
        }
        guard result.status != 0 else { return resolution }
        let remainsRegistered = try await isRegistered(
            resolution: resolution,
            repoPath: repoPath,
            context: context,
            timeout: deadline.remaining()
        )
        guard remainsRegistered else { return resolution }
        if try await context.fileOps.exists(at: path, timeout: deadline.remaining()) {
            throw GitWorktreeError.commandFailed(
                result.stderr.isEmpty ? "Failed to remove worktree." : result.stderr
            )
        }

        if let remaining = try? deadline.remaining() {
            try? await pruneWorktrees(repoPath: repoPath, context: context, timeout: remaining)
        }
        try Task.checkCancellation()
        let stillRegistered = try await isRegistered(
            resolution: resolution,
            repoPath: repoPath,
            context: context,
            timeout: deadline.remaining()
        )
        guard stillRegistered else { return resolution }

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
        resolution: WorktreePathResolution,
        repoPath: String,
        context: WorkspaceContext,
        timeout: TimeInterval
    ) async throws -> Bool {
        let targets = Set(resolution.identityPaths.map {
            Self.normalizedPathIdentity($0, context: context)
        })
        return try await listWorktrees(repoPath: repoPath, context: context, timeout: timeout)
            .contains { targets.contains(Self.normalizedPathIdentity($0.path, context: context)) }
    }

    func isWorktreeRegistered(
        repoPath: String,
        resolution: WorktreePathResolution,
        context: WorkspaceContext,
        deadline: OperationDeadline
    ) async throws -> Bool {
        try await isRegistered(
            resolution: resolution,
            repoPath: repoPath,
            context: context,
            timeout: deadline.remaining()
        )
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
        try await resolveWorktreePath(
            path,
            repoPath: repoPath,
            context: context,
            deadline: OperationDeadline(timeout: timeout)
        )
    }

    static func resolveWorktreePath(
        _ path: String,
        repoPath: String,
        context: WorkspaceContext,
        deadline: OperationDeadline
    ) async throws -> WorktreePathResolution {
        guard case let .ssh(destination) = context else {
            return try await resolveLocalWorktreePaths(
                [path],
                repoPath: repoPath,
                deadline: deadline
            )[0]
        }
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
        return try await resolveRemoteWorktreePaths(
            [path],
            repoPath: repoPath,
            homePath: homePath,
            destination: destination,
            deadline: deadline
        )[0]
    }

    static func resolveRemoteWorktreePaths(
        _ paths: [String],
        repoPath: String,
        homePath: String,
        destination: SSHDestination,
        deadline: OperationDeadline
    ) async throws -> [WorktreePathResolution] {
        guard !paths.isEmpty else { return [] }
        let absolutePaths = paths.map {
            expandedRemotePath($0, repoPath: repoPath, homePath: homePath)
        }
        let result = try await SSHCommandRunner.run(
            destination: destination,
            remoteCommand: remotePathResolutionCommand(absolutePaths),
            timeout: deadline.remaining(),
            outputByteLimit: maximumCanonicalPathOutputByteCount
        )
        guard result.status == 0, !result.truncated else {
            throw GitWorktreeError.commandFailed(
                result.stderr.isEmpty ? "Failed to resolve the remote worktree path." : result.stderr
            )
        }
        let encodedPaths = try decodeCanonicalPathOutput(
            result.stdoutData,
            expectedCount: paths.count
        )
        _ = try deadline.remaining()
        return zip(absolutePaths, encodedPaths).map { absolutePath, encodedPath in
            let resolvedPath = ProjectPickerPathService.standardizedRemotePath(encodedPath)
            return WorktreePathResolution(
                path: resolvedPath,
                identityPaths: [absolutePath, resolvedPath],
                remoteHomePath: homePath
            )
        }
    }

    static func remotePathResolutionCommand(_ absolutePaths: [String]) -> String {
        let arguments = absolutePaths
            .map(RemoteCommandBuilder.quoteRemotePath)
            .joined(separator: " ")
        return """
        set -- \(arguments)
        \(pathResolutionScript)
        """
    }

    private static let pathResolutionScript = """
    for input in "$@"; do
      candidate=$input
      suffix=
      hops=0
      emitted=0
      while :; do
        while [ ! -e "$candidate" ] && [ ! -L "$candidate" ]; do
          [ "$candidate" != / ] || break
          case "$candidate" in
            */*) name=${candidate##*/}; parent=${candidate%/*}; [ -n "$parent" ] || parent=/ ;;
            *) name=$candidate; parent=. ;;
          esac
          suffix=/$name$suffix
          candidate=$parent
        done
        if [ -L "$candidate" ]; then
          link=$(readlink -n "$candidate" && printf '\\001') || break
          link=${link%?}
          hops=$((hops + 1))
          [ "$hops" -le 40 ] || break
          case "$link" in
            /*) candidate=$link ;;
            *)
              case "$candidate" in
                */*) parent=${candidate%/*}; [ -n "$parent" ] || parent=/ ;;
                *) parent=. ;;
              esac
              candidate=$parent/$link
              ;;
          esac
          continue
        fi
        if [ -d "$candidate" ]; then
          if (cd -P "$candidate" 2>/dev/null && printf '%s%s\\000' "$PWD" "$suffix"); then
            emitted=1
          fi
          break
        fi
        [ -e "$candidate" ] || break
        case "$candidate" in
          */*) name=${candidate##*/}; parent=${candidate%/*}; [ -n "$parent" ] || parent=/ ;;
          *) name=$candidate; parent=. ;;
        esac
        suffix=/$name$suffix
        candidate=$parent
      done
      [ "$emitted" -eq 1 ] || printf '%s\\000' "$input"
    done
    """

    static func resolveLocalWorktreePaths(
        _ paths: [String],
        repoPath: String,
        deadline: OperationDeadline
    ) async throws -> [WorktreePathResolution] {
        guard !paths.isEmpty else { return [] }
        let expandedPaths = paths.map { NSString(string: $0).expandingTildeInPath }
        let hasRelativePath = expandedPaths.contains { !$0.hasPrefix("/") }
        let resolvedRepository: String? = if hasRelativePath {
            try await canonicalLocalPaths(
                [NSString(string: repoPath).expandingTildeInPath],
                deadline: deadline
            )[0]
        } else {
            nil
        }
        let absolutePaths = paths.map { absoluteLocalPath($0, repoPath: repoPath) }
        let resolutionInputs = zip(expandedPaths, absolutePaths).map { expandedPath, absolutePath in
            guard !expandedPath.hasPrefix("/"), let resolvedRepository else { return absolutePath }
            return URL(fileURLWithPath: resolvedRepository)
                .appendingPathComponent(expandedPath)
                .standardizedFileURL
                .path
        }
        let resolvedPaths = try await canonicalLocalPaths(resolutionInputs, deadline: deadline)
        return zip(zip(absolutePaths, resolutionInputs), resolvedPaths).map { inputs, resolvedPath in
            WorktreePathResolution(
                path: resolvedPath,
                identityPaths: [inputs.0, inputs.1, resolvedPath],
                remoteHomePath: nil
            )
        }
    }

    static func canonicalLocalPaths(_ paths: [String], deadline: OperationDeadline) async throws -> [String] {
        guard !paths.isEmpty else { return [] }
        let result = try await SubprocessRunner.run(SubprocessRequest(
            executablePath: "/bin/zsh",
            arguments: [
                "-f",
                "-c",
                pathResolutionScript,
                "muxy-resolve-paths",
            ] + paths,
            timeout: deadline.remaining(),
            outputByteLimit: maximumCanonicalPathOutputByteCount
        ))
        guard result.status == 0, !result.truncated else {
            throw GitWorktreeError.commandFailed("Failed to resolve the local worktree path.")
        }
        let resolvedPaths = try decodeCanonicalPathOutput(
            result.stdoutData,
            expectedCount: paths.count
        )
        _ = try deadline.remaining()
        return resolvedPaths
    }

    static func decodeCanonicalPathOutput(_ data: Data, expectedCount: Int) throws -> [String] {
        var encodedPaths = data.split(separator: 0, omittingEmptySubsequences: false)
        guard encodedPaths.last?.isEmpty == true else {
            throw GitWorktreeError.commandFailed("Failed to decode resolved worktree paths.")
        }
        encodedPaths.removeLast()
        let resolvedPaths = encodedPaths.compactMap { String(data: Data($0), encoding: .utf8) }
        guard resolvedPaths.count == expectedCount else {
            throw GitWorktreeError.commandFailed("Failed to decode resolved worktree paths.")
        }
        return resolvedPaths
    }

    static func absoluteLocalPath(_ path: String, repoPath: String) -> String {
        let expandedPath = NSString(string: path).expandingTildeInPath
        guard !expandedPath.hasPrefix("/") else {
            return URL(fileURLWithPath: expandedPath).standardizedFileURL.path
        }
        let expandedRepoPath = NSString(string: repoPath).expandingTildeInPath
        let repository = URL(fileURLWithPath: expandedRepoPath).standardizedFileURL
        return repository.appendingPathComponent(expandedPath).standardizedFileURL.path
    }

    static func normalizedPathIdentity(_ path: String, context: WorkspaceContext) -> String {
        guard !context.isRemote else { return ProjectPickerPathService.standardizedRemotePath(path) }
        return URL(fileURLWithPath: path).standardizedFileURL.path
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
