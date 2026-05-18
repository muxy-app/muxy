import Foundation
import Testing

@testable import Muxy

@Suite("GitWorktreeService.addWorktree")
struct GitWorktreeServiceAddTests {
    @Test("new branch is created from the supplied base branch, not current HEAD")
    func newBranchUsesBaseBranch() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }

        try repo.commit(file: "a.txt", contents: "1", message: "base")
        try repo.run("checkout", "-b", "release")
        try repo.commit(file: "b.txt", contents: "2", message: "on release")
        try repo.run("checkout", "main")

        let worktreePath = repo.siblingPath("feature-wt")
        try await GitWorktreeService.shared.addWorktree(
            repoPath: repo.path,
            path: worktreePath,
            branch: "feature",
            createBranch: true,
            baseBranch: "release"
        )

        let head = try repo.runCapturing(at: worktreePath, "rev-parse", "HEAD")
        let releaseHead = try repo.runCapturing("rev-parse", "release")
        #expect(head == releaseHead)

        let bExists = FileManager.default.fileExists(atPath: "\(worktreePath)/b.txt")
        #expect(bExists, "Expected file from release branch to be present in the new worktree")
    }

    @Test("omitting the base branch falls back to current HEAD")
    func newBranchDefaultsToHEADWhenNoBase() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }

        try repo.commit(file: "a.txt", contents: "1", message: "base")

        let worktreePath = repo.siblingPath("noBase-wt")
        try await GitWorktreeService.shared.addWorktree(
            repoPath: repo.path,
            path: worktreePath,
            branch: "feature",
            createBranch: true,
            baseBranch: nil
        )

        let head = try repo.runCapturing(at: worktreePath, "rev-parse", "HEAD")
        let mainHead = try repo.runCapturing("rev-parse", "main")
        #expect(head == mainHead)
    }
}

private struct TempGitRepo {
    let path: String
    private let parent: String

    init() throws {
        let base = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-worktree-tests-\(UUID().uuidString)", isDirectory: true)
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

    func run(_ args: String...) throws {
        _ = try runGit(at: path, args: args)
    }

    func runCapturing(_ args: String...) throws -> String {
        try runGit(at: path, args: args)
    }

    func runCapturing(at workingDir: String, _ args: String...) throws -> String {
        try runGit(at: workingDir, args: args)
    }

    private func runGit(at workingDir: String, args: [String]) throws -> String {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["git", "-C", workingDir] + args
        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = pipe
        try process.run()
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        let output = String(data: data, encoding: .utf8) ?? ""
        if process.terminationStatus != 0 {
            throw NSError(
                domain: "GitTestRepo",
                code: Int(process.terminationStatus),
                userInfo: [NSLocalizedDescriptionKey: output]
            )
        }
        return output.trimmingCharacters(in: .whitespacesAndNewlines)
    }
}
