import Foundation
import Testing

@testable import Muxy

@Suite("GitWorktreeService.removeWorktree")
struct GitWorktreeServiceRemoveTests {
    @Test("removes a normal worktree")
    func removesNormalWorktree() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }

        try repo.commit(file: "a.txt", contents: "1", message: "base")
        let worktreePath = repo.siblingPath("feature-wt")
        try await GitWorktreeService.shared.addWorktree(
            repoPath: repo.path,
            path: worktreePath,
            branch: "feature",
            createBranch: true,
            baseBranch: nil
        )

        try await GitWorktreeService.shared.removeWorktree(repoPath: repo.path, path: worktreePath, force: true)

        let records = try await GitWorktreeService.shared.listWorktrees(repoPath: repo.path)
        #expect(!records.contains { $0.path == worktreePath })
    }

    @Test("succeeds when the worktree folder is gone and git admin metadata is orphaned")
    func succeedsForOrphanedWorktree() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }

        try repo.commit(file: "a.txt", contents: "1", message: "base")
        let worktreePath = repo.siblingPath("orphan-wt")
        try await GitWorktreeService.shared.addWorktree(
            repoPath: repo.path,
            path: worktreePath,
            branch: "feature",
            createBranch: true,
            baseBranch: nil
        )
        try FileManager.default.removeItem(atPath: worktreePath)
        try repo.orphanWorktreeAdmin(named: "orphan-wt")

        try await GitWorktreeService.shared.removeWorktree(repoPath: repo.path, path: worktreePath, force: true)

        let records = try await GitWorktreeService.shared.listWorktrees(repoPath: repo.path)
        #expect(!records.contains { $0.path == worktreePath })
    }

    @Test("cleanupOnDisk removes the worktree but keeps its branch")
    func cleanupKeepsBranch() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }

        try repo.commit(file: "a.txt", contents: "1", message: "base")
        let worktreePath = repo.siblingPath("keep-branch-wt")
        try await GitWorktreeService.shared.addWorktree(
            repoPath: repo.path,
            path: worktreePath,
            branch: "feature",
            createBranch: true,
            baseBranch: nil
        )

        let worktree = Worktree(name: "keep-branch-wt", path: worktreePath, branch: "feature", isPrimary: false)
        try await WorktreeStore.cleanupOnDisk(worktree: worktree, repoPath: repo.path)

        let records = try await GitWorktreeService.shared.listWorktrees(repoPath: repo.path)
        #expect(!records.contains { $0.path == worktreePath })
        #expect(repo.branchExists("feature"))
    }

    @Test("heals an orphaned worktree referenced through a symlinked parent")
    func healsOrphanThroughSymlinkedParent() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }

        try repo.commit(file: "a.txt", contents: "1", message: "base")
        let worktreePath = repo.siblingPath("symlink-wt")
        try await GitWorktreeService.shared.addWorktree(
            repoPath: repo.path,
            path: worktreePath,
            branch: "feature",
            createBranch: true,
            baseBranch: nil
        )
        try FileManager.default.removeItem(atPath: worktreePath)
        try repo.orphanWorktreeAdmin(named: "symlink-wt")

        let aliasPath = try repo.symlinkedSiblingPath(for: "symlink-wt")
        try await GitWorktreeService.shared.removeWorktree(repoPath: repo.path, path: aliasPath, force: true)

        let records = try await GitWorktreeService.shared.listWorktrees(repoPath: repo.path)
        #expect(!records.contains { GitWorktreeService.canonicalPath($0.path) == GitWorktreeService.canonicalPath(worktreePath) })
    }
}

private struct TempGitRepo {
    let path: String
    private let parent: String

    init() throws {
        let base = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-worktree-remove-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: base, withIntermediateDirectories: true)
        parent = base.path
        path = base.appendingPathComponent("repo", isDirectory: true).path
        try FileManager.default.createDirectory(atPath: path, withIntermediateDirectories: true)
        try run("init", "-q", "-b", "main")
        try run("config", "user.email", "test@example.com")
        try run("config", "user.name", "Test")
        try run("config", "commit.gpgsign", "false")
    }

    func cleanup() {
        try? FileManager.default.removeItem(atPath: parent)
    }

    func siblingPath(_ name: String) -> String {
        URL(fileURLWithPath: parent).appendingPathComponent(name).path
    }

    func commit(file: String, contents: String, message: String) throws {
        let fileURL = URL(fileURLWithPath: path).appendingPathComponent(file)
        try contents.write(to: fileURL, atomically: true, encoding: .utf8)
        try run("add", file)
        try run("commit", "-q", "-m", message)
    }

    func symlinkedSiblingPath(for name: String) throws -> String {
        let realParent = URL(fileURLWithPath: parent)
        let aliasParent = realParent.appendingPathComponent("alias-\(UUID().uuidString)")
        try FileManager.default.createSymbolicLink(at: aliasParent, withDestinationURL: realParent)
        return aliasParent.appendingPathComponent(name).path
    }

    func branchExists(_ branch: String) -> Bool {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["git", "-C", path, "branch", "--list", "--format=%(refname:short)"]
        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = Pipe()
        guard (try? process.run()) != nil else { return false }
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        let output = String(data: data, encoding: .utf8) ?? ""
        return output.split(separator: "\n").map { $0.trimmingCharacters(in: .whitespaces) }.contains(branch)
    }

    func orphanWorktreeAdmin(named name: String) throws {
        let gitdir = URL(fileURLWithPath: path)
            .appendingPathComponent(".git/worktrees/\(name)/gitdir")
        try "/nonexistent/\(name)/.git\n".write(to: gitdir, atomically: true, encoding: .utf8)
    }

    func run(_ args: String...) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["git", "-C", path] + args
        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = pipe
        try process.run()
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else {
            throw NSError(
                domain: "GitTestRepo",
                code: Int(process.terminationStatus),
                userInfo: [NSLocalizedDescriptionKey: String(data: data, encoding: .utf8) ?? ""]
            )
        }
    }
}
