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
    static let shared = GitWorktreeService()

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

    func isGitRepository(_ path: String, context: WorkspaceContext = .local) async -> Bool {
        guard let result = try? await GitProcessRunner.runGit(
            repoPath: path,
            arguments: ["rev-parse", "--is-inside-work-tree"],
            context: context
        )
        else {
            return false
        }
        return result.status == 0 && result.stdout.trimmingCharacters(in: .whitespacesAndNewlines) == "true"
    }

    func hasUncommittedChanges(worktreePath: String, context: WorkspaceContext = .local) async -> Bool {
        guard let result = try? await GitProcessRunner.runGit(
            repoPath: worktreePath,
            arguments: ["status", "--porcelain=1", "--untracked-files=all"],
            context: context
        )
        else {
            return false
        }
        guard result.status == 0 else { return false }
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
        context: WorkspaceContext = .local
    ) async throws {
        var args: [String] = ["worktree", "remove"]
        if force {
            args.append("--force")
        }
        args += ["--", path]
        let result = try await GitProcessRunner.runGit(repoPath: repoPath, arguments: args, context: context)

        try? await pruneWorktrees(repoPath: repoPath, context: context)
        let target = Self.canonicalPath(path, context: context)
        let stillRegistered = try await listWorktrees(repoPath: repoPath, context: context)
            .contains { Self.canonicalPath($0.path, context: context) == target }
        guard stillRegistered else { return }

        throw GitWorktreeError.commandFailed(
            result.stderr.isEmpty ? "Failed to remove worktree." : result.stderr
        )
    }

    private func pruneWorktrees(repoPath: String, context: WorkspaceContext) async throws {
        let result = try await GitProcessRunner.runGit(
            repoPath: repoPath,
            arguments: ["worktree", "prune"],
            context: context
        )
        guard result.status == 0 else {
            throw GitWorktreeError.commandFailed(
                result.stderr.isEmpty ? "Failed to prune worktrees." : result.stderr
            )
        }
    }

    static func canonicalPath(_ path: String, context: WorkspaceContext = .local) -> String {
        guard !context.isRemote else { return path }
        let standardized = URL(fileURLWithPath: path).standardizedFileURL
        let resolved = standardized.resolvingSymlinksInPath()
        guard resolved.path == standardized.path else { return resolved.path }

        let parent = standardized.deletingLastPathComponent().resolvingSymlinksInPath()
        return parent.appendingPathComponent(standardized.lastPathComponent).path
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
