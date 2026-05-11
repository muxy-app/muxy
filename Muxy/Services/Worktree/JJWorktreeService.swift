import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "JJWorktreeService")

enum JJWorktreeError: LocalizedError {
    case commandFailed(String)

    var errorDescription: String? {
        switch self {
        case let .commandFailed(message):
            message
        }
    }
}

actor JJWorktreeService: WorktreeService {
    static let shared = JJWorktreeService()

    func isRepository(_ path: String) async -> Bool {
        let kind = await VCSKind.detect(at: path)
        return kind?.isJujutsu ?? false
    }

    func hasUncommittedChanges(worktreePath: String) async -> Bool {
        guard let result = try? await runJJ(
            repoPath: worktreePath,
            arguments: ["diff", "--summary"]
        )
        else {
            return false
        }
        guard result.status == 0 else { return false }
        return !result.stdout.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    func listWorktrees(repoPath: String) async throws -> [WorktreeRecord] {
        let kind = await VCSKind.detect(at: repoPath)
        guard kind?.isJujutsu ?? false else {
            return []
        }

        let result = try await runJJ(
            repoPath: repoPath,
            arguments: ["workspace", "list"]
        )
        guard result.status == 0 else {
            throw JJWorktreeError.commandFailed(
                result.stderr.isEmpty ? "Failed to list workspaces." : result.stderr
            )
        }

        let lines = result.stdout
            .split(separator: "\n")
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }

        var records: [WorktreeRecord] = []
        for line in lines {
            let name = String(line.prefix { $0 != ":" }).trimmingCharacters(in: .whitespaces)
            guard !name.isEmpty else { continue }

            let rootResult = try? await runJJ(
                repoPath: repoPath,
                arguments: ["workspace", "root", "--name", name]
            )
            guard let rootResult, rootResult.status == 0 else { continue }

            let path = rootResult.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !path.isEmpty else { continue }

            let branch: String? = {
                guard let colonIndex = line.firstIndex(of: ":") else { return nil }
                let afterColon = line[line.index(after: colonIndex)...]
                let trimmed = afterColon.trimmingCharacters(in: .whitespaces)
                return trimmed.isEmpty ? nil : trimmed
            }()

            records.append(WorktreeRecord(
                name: name,
                path: path,
                branch: branch,
                head: nil,
                isBare: false,
                isDetached: false,
                isPrunable: false
            ))
        }

        return records
    }

    func addWorktree(repoPath: String, path: String, branch: String, createBranch: Bool) async throws {
        let name = URL(fileURLWithPath: path).lastPathComponent
        guard !name.isEmpty else {
            throw JJWorktreeError.commandFailed("Invalid workspace name.")
        }

        var args: [String] = ["workspace", "add", "--name", name]
        let trimmedBranch = branch.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmedBranch.isEmpty {
            args += ["-r", trimmedBranch]
        }
        args.append(path)

        let result = try await runJJ(repoPath: repoPath, arguments: args)
        guard result.status == 0 else {
            throw JJWorktreeError.commandFailed(
                result.stderr.isEmpty ? "Failed to add workspace." : result.stderr
            )
        }
    }

    func removeWorktree(repoPath: String, path: String, force: Bool) async throws {
        let name = URL(fileURLWithPath: path).lastPathComponent
        guard !name.isEmpty else {
            throw JJWorktreeError.commandFailed("Invalid workspace name.")
        }

        let result = try await runJJ(
            repoPath: repoPath,
            arguments: ["workspace", "forget", name]
        )
        guard result.status == 0 else {
            throw JJWorktreeError.commandFailed(
                result.stderr.isEmpty ? "Failed to remove workspace." : result.stderr
            )
        }

        try? FileManager.default.removeItem(atPath: path)
    }

    private func runJJ(repoPath: String, arguments: [String]) async throws -> GitProcessResult {
        try await GitProcessRunner.runCommand(
            executable: "/usr/bin/env",
            arguments: ["jj", "-R", repoPath] + arguments,
            workingDirectory: repoPath
        )
    }
}
