import Foundation

struct GitWorktreeRecord: Hashable {
    let path: String
    let branch: String?
    let head: String?
    let isBare: Bool
    let isDetached: Bool
}

actor GitWorktreeService {
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

    func isGitRepository(_ path: String) async -> Bool {
        guard let result = try? runGit(repoPath: path, arguments: ["rev-parse", "--is-inside-work-tree"]) else {
            return false
        }
        return result.status == 0 && result.stdout.trimmingCharacters(in: .whitespacesAndNewlines) == "true"
    }

    func listWorktrees(repoPath: String) async throws -> [GitWorktreeRecord] {
        let result = try runGit(repoPath: repoPath, arguments: ["worktree", "list", "--porcelain"])
        guard result.status == 0 else {
            throw GitWorktreeError.commandFailed(
                result.stderr.isEmpty ? "Failed to list worktrees." : result.stderr
            )
        }
        return parsePorcelain(result.stdout)
    }

    func addWorktree(repoPath: String, path: String, branch: String, createBranch: Bool) async throws {
        guard !branch.isEmpty else {
            throw GitWorktreeError.commandFailed("Branch name is required.")
        }
        var args: [String] = ["worktree", "add"]
        if createBranch {
            args += ["-b", branch, path]
        } else {
            args += [path, branch]
        }
        let result = try runGit(repoPath: repoPath, arguments: args)
        guard result.status == 0 else {
            throw GitWorktreeError.commandFailed(
                result.stderr.isEmpty ? "Failed to add worktree." : result.stderr
            )
        }
    }

    func removeWorktree(repoPath: String, path: String, force: Bool = false) async throws {
        var args: [String] = ["worktree", "remove"]
        if force { args.append("--force") }
        args.append(path)
        let result = try runGit(repoPath: repoPath, arguments: args)
        guard result.status == 0 else {
            throw GitWorktreeError.commandFailed(
                result.stderr.isEmpty ? "Failed to remove worktree." : result.stderr
            )
        }
    }

    private func parsePorcelain(_ raw: String) -> [GitWorktreeRecord] {
        var records: [GitWorktreeRecord] = []
        var currentPath: String?
        var currentBranch: String?
        var currentHead: String?
        var isBare = false
        var isDetached = false

        func flush() {
            guard let path = currentPath else { return }
            records.append(GitWorktreeRecord(
                path: path,
                branch: currentBranch,
                head: currentHead,
                isBare: isBare,
                isDetached: isDetached
            ))
            currentPath = nil
            currentBranch = nil
            currentHead = nil
            isBare = false
            isDetached = false
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
            }
        }
        flush()
        return records
    }

    private struct GitRunResult {
        let status: Int32
        let stdout: String
        let stderr: String
    }

    private func runGit(repoPath: String, arguments: [String]) throws -> GitRunResult {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["git", "-C", repoPath] + arguments

        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe

        try process.run()
        let stdoutData = stdoutPipe.fileHandleForReading.readDataToEndOfFile()
        let stderrData = stderrPipe.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()

        let stdout = String(data: stdoutData, encoding: .utf8) ?? ""
        let stderr = String(data: stderrData, encoding: .utf8) ?? ""
        return GitRunResult(status: process.terminationStatus, stdout: stdout, stderr: stderr)
    }
}
