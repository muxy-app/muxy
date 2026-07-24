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

    @Test("uses the branch advertised by the remote HEAD")
    func parsesRemoteDefaultBranch() {
        let branch = GitRepositoryService.defaultBranch(fromRemoteHEAD: """
        ref: refs/heads/develop\tHEAD
        6afb7d6642b93be44dddd5b13d3eb39901142f18\tHEAD
        """)

        #expect(branch == "develop")
    }

    @Test("ignores a remote HEAD response without a symbolic branch")
    func ignoresRemoteDefaultBranchWithoutSymbolicRef() {
        let branch = GitRepositoryService.defaultBranch(
            fromRemoteHEAD: "6afb7d6642b93be44dddd5b13d3eb39901142f18\tHEAD"
        )

        #expect(branch == nil)
    }

    @Test("remote HEAD overrides stale local default branch metadata")
    func remoteDefaultBranchOverridesStaleLocalMetadata() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }
        let remotePath = try repo.createBareRemote()
        try repo.run("remote", "add", "origin", remotePath)
        try repo.commit(file: "main.txt", contents: "main", message: "main")
        try repo.run("push", "-u", "origin", "main")
        try repo.run("checkout", "-b", "develop")
        try repo.commit(file: "develop.txt", contents: "develop", message: "develop")
        try repo.run("push", "-u", "origin", "develop")
        try repo.run("checkout", "main")
        try repo.run("remote", "set-head", "origin", "main")
        try repo.setRemoteHEAD(remotePath, branch: "develop")

        #expect(try repo.runCapturing("symbolic-ref", "--short", "refs/remotes/origin/HEAD") == "origin/main")
        #expect(await GitRepositoryService().defaultBranch(repoPath: repo.path) == "develop")
    }

    @Test("local default branch metadata is used when remote HEAD is not symbolic")
    func localDefaultBranchIsUsedWhenRemoteHEADIsNotSymbolic() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }
        let remotePath = try repo.createBareRemote()
        try repo.run("remote", "add", "origin", remotePath)
        try repo.commit(file: "main.txt", contents: "main", message: "main")
        try repo.run("push", "-u", "origin", "main")
        try repo.run("remote", "set-head", "origin", "main")
        try repo.detachRemoteHEAD(remotePath, commit: try repo.runCapturing("rev-parse", "main"))

        #expect(await GitRepositoryService().defaultBranch(repoPath: repo.path) == "main")
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

    @Test("deleteLocalBranch protects checked-out branches and force deletes unmerged branches")
    func deleteLocalBranch() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }
        try repo.commit(file: "a.txt", contents: "1", message: "init")
        try repo.run("branch", "merged")

        let service = GitRepositoryService()
        await #expect(throws: Error.self) {
            try await service.deleteLocalBranch(repoPath: repo.path, branch: "main", force: true)
        }
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

    @Test("deleteLocalBranch rejects a branch checked out in another worktree")
    func deleteBranchCheckedOutInAnotherWorktree() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }
        try repo.commit(file: "a.txt", contents: "1", message: "init")
        let worktreePath = repo.sibling("linked")
        try repo.run("worktree", "add", "-b", "linked-branch", worktreePath)

        let service = GitRepositoryService()
        await #expect(throws: Error.self) {
            try await service.deleteLocalBranch(repoPath: repo.path, branch: "linked-branch", force: true)
        }

        #expect(try await service.listBranches(repoPath: repo.path).contains("linked-branch"))
    }

    @Test("createAndSwitchBranch creates from HEAD and keeps failures on the current branch")
    func createAndSwitchBranch() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }
        try repo.commit(file: "a.txt", contents: "1", message: "init")
        let service = GitRepositoryService()

        try await service.createAndSwitchBranch(repoPath: repo.path, name: "feature/inline-branches")

        #expect(try await service.currentBranch(repoPath: repo.path) == "feature/inline-branches")
        #expect(try await service.listBranches(repoPath: repo.path) == ["feature/inline-branches", "main"])
        await #expect(throws: Error.self) {
            try await service.createAndSwitchBranch(repoPath: repo.path, name: "feature/inline-branches")
        }
        await #expect(throws: Error.self) {
            try await service.createAndSwitchBranch(repoPath: repo.path, name: "invalid..branch")
        }
        #expect(try await service.currentBranch(repoPath: repo.path) == "feature/inline-branches")
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

    @Test("stage and unstage rename include both repository paths")
    func stageAndUnstageRename() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }
        try repo.commit(file: "old.txt", contents: "content\n", message: "init")
        try FileManager.default.moveItem(
            atPath: URL(fileURLWithPath: repo.path).appendingPathComponent("old.txt").path,
            toPath: URL(fileURLWithPath: repo.path).appendingPathComponent("new.txt").path
        )
        try repo.run("add", "-A")

        let service = GitRepositoryService()
        let stagedRename = try #require(try await service.changedFiles(repoPath: repo.path).first {
            $0.oldPath == "old.txt" && $0.path == "new.txt" && $0.isStaged
        })
        try await service.unstageFiles(repoPath: repo.path, paths: stagedRename.relatedPaths)

        #expect(try await service.changedFiles(repoPath: repo.path).allSatisfy { !$0.isStaged })

        try await service.stageFiles(repoPath: repo.path, paths: stagedRename.relatedPaths)

        #expect(try await service.changedFiles(repoPath: repo.path).contains {
            $0.oldPath == "old.txt" && $0.path == "new.txt" && $0.isStaged
        })
    }

    @Test("discard request permanently removes an untracked file")
    func discardUntrackedFile() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }
        try repo.commit(file: "tracked.txt", contents: "content\n", message: "init")
        let newFile = URL(fileURLWithPath: repo.path).appendingPathComponent("new.txt")
        try "new\n".write(to: newFile, atomically: true, encoding: .utf8)

        let service = GitRepositoryService()
        let file = try #require(try await service.changedFiles(repoPath: repo.path).first { $0.path == "new.txt" })
        let request = try #require(RepositoryChangesPresentation.discardRequest(file))
        try await service.discardFiles(
            repoPath: repo.path,
            paths: request.paths,
            untrackedPaths: request.untrackedPaths
        )

        #expect(!FileManager.default.fileExists(atPath: newFile.path))
        #expect(try await service.changedFiles(repoPath: repo.path).isEmpty)
    }

    @Test("untracked line count does not add a line after a trailing newline")
    func untrackedLineCountWithTrailingNewline() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }
        try repo.commit(file: "tracked.txt", contents: "content\n", message: "init")
        try "new\n".write(
            to: URL(fileURLWithPath: repo.path).appendingPathComponent("new.txt"),
            atomically: true,
            encoding: .utf8
        )

        let file = try #require(
            try await GitRepositoryService().changedFiles(repoPath: repo.path).first { $0.path == "new.txt" }
        )

        #expect(file.additions == 1)
        #expect(file.deletions == 0)
    }

    @Test("untracked line counts can load after the initial status")
    func untrackedLineCountsLoadOnDemand() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }
        try repo.commit(file: "tracked.txt", contents: "content\n", message: "init")
        try "one\ntwo\n".write(
            to: URL(fileURLWithPath: repo.path).appendingPathComponent("new.txt"),
            atomically: true,
            encoding: .utf8
        )

        let service = GitRepositoryService()
        let initialFile = try #require(
            try await service.changedFiles(
                repoPath: repo.path,
                includeUntrackedLineCounts: false
            ).first { $0.path == "new.txt" }
        )

        #expect(initialFile.additions == nil)
        #expect(try await service.untrackedFileLineCount(repoPath: repo.path, path: "new.txt") == 2)
    }

    @Test("untracked line counting is bounded for large files")
    func untrackedLineCountingIsBounded() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }
        try repo.commit(file: "tracked.txt", contents: "content\n", message: "init")
        let largeFile = URL(fileURLWithPath: repo.path).appendingPathComponent("large.txt")
        try Data(repeating: 0x61, count: 1_048_577).write(to: largeFile)

        let lineCount = try await GitRepositoryService().untrackedFileLineCount(
            repoPath: repo.path,
            path: "large.txt"
        )

        #expect(lineCount == nil)
    }

    @Test("untracked line counting rejects invalid UTF-8")
    func untrackedLineCountingRejectsInvalidUTF8() async throws {
        let repo = try TempGitRepo()
        defer { repo.cleanup() }
        try repo.commit(file: "tracked.txt", contents: "content\n", message: "init")
        let invalidFile = URL(fileURLWithPath: repo.path).appendingPathComponent("invalid.txt")
        try Data([0xFF, 0x0A]).write(to: invalidFile)

        let lineCount = try await GitRepositoryService().untrackedFileLineCount(
            repoPath: repo.path,
            path: "invalid.txt"
        )

        #expect(lineCount == nil)
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

    func createBareRemote() throws -> String {
        let remotePath = sibling("remote.git")
        try Self.runGit(at: parent, args: ["init", "-q", "--bare", "-b", "main", remotePath])
        return remotePath
    }

    func setRemoteHEAD(_ remotePath: String, branch: String) throws {
        try Self.runGit(at: remotePath, args: ["symbolic-ref", "HEAD", "refs/heads/\(branch)"])
    }

    func detachRemoteHEAD(_ remotePath: String, commit: String) throws {
        try Self.runGit(at: remotePath, args: ["update-ref", "--no-deref", "HEAD", commit])
    }

    func run(_ args: String...) throws {
        try Self.runGit(at: path, args: args)
    }

    func runCapturing(_ args: String...) throws -> String {
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
