import Foundation
import Testing

@testable import Muxy

@Suite("GitRepositoryService new extension APIs")
struct GitRepositoryServiceNewAPIsTests {
    @Test("resolves GitHub CLI locally from the local executable search")
    func resolvesLocalGitHubCLI() {
        let resolved = GitRepositoryService.resolveGhExecutable(
            context: .local,
            localResolver: { executable in executable == "gh" ? "/local/bin/gh" : nil }
        )

        #expect(resolved == "/local/bin/gh")
    }

    @Test("reports a missing local GitHub CLI")
    func reportsMissingLocalGitHubCLI() {
        let resolved = GitRepositoryService.resolveGhExecutable(
            context: .local,
            localResolver: { _ in nil }
        )

        #expect(resolved == nil)
    }

    @Test("uses the remote GitHub CLI name for SSH workspaces")
    func resolvesRemoteGitHubCLI() {
        let context = WorkspaceContext.ssh(SSHDestination(host: "example.test"))
        let resolved = GitRepositoryService.resolveGhExecutable(
            context: context,
            localResolver: { _ in nil }
        )

        #expect(resolved == "gh")
    }

    @Test("repoInfo reports root, gitDir and current branch for a normal repo")
    func repoInfoForNormalRepo() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }
        try repo.commit(file: "a.txt", contents: "1", message: "init")

        let info = try await GitRepositoryService().repoInfo(repoPath: repo.path)

        #expect(repo.isSameDirectory(info.root))
        #expect(!info.isWorktree)
        #expect(info.currentBranch == "main")
        #expect(info.gitDir.hasSuffix(".git"))
    }

    @Test("repoInfo flags a linked worktree")
    func repoInfoForWorktree() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }
        try repo.commit(file: "a.txt", contents: "1", message: "init")
        let worktreePath = repo.sibling("wt")
        try repo.run("worktree", "add", "-b", "feature", worktreePath)

        let info = try await GitRepositoryService().repoInfo(repoPath: worktreePath)

        #expect(info.isWorktree)
        #expect(info.currentBranch == "feature")
    }

    @Test("deleteLocalBranch removes a merged branch and rejects unmerged without force")
    func deleteLocalBranch() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }
        try repo.commit(file: "a.txt", contents: "1", message: "init")
        try repo.run("branch", "merged")

        let service = GitRepositoryService()
        try await service.deleteLocalBranch(repoPath: repo.path, branch: "merged", force: false)
        let branches = try await service.listBranches(repoPath: repo.path)
        #expect(!branches.contains("merged"))

        try repo.run("checkout", "-b", "unmerged")
        try repo.commit(file: "b.txt", contents: "1", message: "extra")
        try repo.run("checkout", "main")
        await #expect(throws: Error.self) {
            try await service.deleteLocalBranch(repoPath: repo.path, branch: "unmerged", force: false)
        }
        try await service.deleteLocalBranch(repoPath: repo.path, branch: "unmerged", force: true)
        #expect(!(try await service.listBranches(repoPath: repo.path)).contains("unmerged"))
    }

    @Test("initRepository turns a plain folder into a git repo")
    func initRepository() async throws {
        let base = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-git-init-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: base, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: base) }

        try await GitRepositoryService().initRepository(repoPath: base.path)
        #expect(FileManager.default.fileExists(atPath: base.appendingPathComponent(".git").path))
    }

    @Test("rawDiff returns unified diff text for the working tree")
    func rawDiffWorkingTree() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }
        try repo.commit(file: "a.txt", contents: "one\n", message: "init")
        try "one\ntwo\n".write(
            to: URL(fileURLWithPath: repo.path).appendingPathComponent("a.txt"),
            atomically: true,
            encoding: .utf8
        )

        let result = try await GitRepositoryService().rawDiff(
            repoPath: repo.path,
            filePath: "a.txt",
            range: nil,
            staged: false,
            lineLimit: nil
        )

        #expect(result.diff.contains("+two"))
        #expect(!result.truncated)
    }

    @Test("repoSignature changes when the working tree advances")
    func repoSignatureChanges() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }
        try repo.commit(file: "a.txt", contents: "1", message: "init")

        let service = GitRepositoryService()
        let first = await service.repoSignature(repoPath: repo.path)
        try repo.commit(file: "b.txt", contents: "1", message: "second")
        let second = await service.repoSignature(repoPath: repo.path)

        #expect(first != second)
    }

    @Test("repoSignature reflects a stage in a linked worktree (real index dir)")
    func repoSignatureTracksWorktreeIndex() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }
        try repo.commit(file: "a.txt", contents: "1", message: "init")
        let worktreePath = repo.sibling("wt")
        try repo.run("worktree", "add", "-b", "feature", worktreePath)

        let service = GitRepositoryService()
        let before = await service.repoSignature(repoPath: worktreePath)
        try "new\n".write(
            to: URL(fileURLWithPath: worktreePath).appendingPathComponent("staged.txt"),
            atomically: true,
            encoding: .utf8
        )
        try Self.runGit(at: worktreePath, args: ["add", "staged.txt"])
        let after = await service.repoSignature(repoPath: worktreePath)

        #expect(before != after)
    }

    @discardableResult
    private static func runGit(at workingDir: String, args: [String]) throws -> String {
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

private struct TempGitRepo {
    let path: String
    private let parent: String

    func isSameDirectory(_ other: String) -> Bool {
        let lhs = try? FileManager.default.attributesOfItem(atPath: path)[.systemFileNumber] as? Int
        let rhs = try? FileManager.default.attributesOfItem(atPath: other)[.systemFileNumber] as? Int
        return lhs != nil && lhs == rhs
    }

    init() throws {
        let base = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-git-new-apis-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: base, withIntermediateDirectories: true)
        parent = base.path
        path = base.appendingPathComponent("repo", isDirectory: true).path
        try Self.runGit(at: parent, args: ["init", "-q", "-b", "main", path])
        try Self.runGit(at: path, args: ["config", "user.email", "test@example.com"])
        try Self.runGit(at: path, args: ["config", "user.name", "Test"])
        try Self.runGit(at: path, args: ["config", "commit.gpgsign", "false"])
    }

    func cleanup() {
        try? FileManager.default.removeItem(atPath: parent)
    }

    func sibling(_ name: String) -> String {
        URL(fileURLWithPath: parent).appendingPathComponent(name).path
    }

    func run(_ args: String...) throws {
        try Self.runGit(at: path, args: args)
    }

    func commit(file: String, contents: String, message: String) throws {
        let fileURL = URL(fileURLWithPath: path).appendingPathComponent(file)
        try contents.write(to: fileURL, atomically: true, encoding: .utf8)
        try Self.runGit(at: path, args: ["add", file])
        try Self.runGit(at: path, args: ["commit", "-q", "-m", message])
    }

    @discardableResult
    private static func runGit(at workingDir: String, args: [String]) throws -> String {
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
