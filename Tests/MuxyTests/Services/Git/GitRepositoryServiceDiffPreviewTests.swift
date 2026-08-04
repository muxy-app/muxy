import Foundation
import Testing

@testable import Muxy

@Suite("GitRepositoryService diff preview")
struct GitRepositoryServiceDiffPreviewTests {
    @Test("untracked preview reads only limited lines")
    func untrackedPreviewReadsOnlyLimitedLines() async throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }

        let fileName = "large.txt"
        let fileURL = directory.appendingPathComponent(fileName)
        let content = (0 ..< 2_500).map { "line \($0)" }.joined(separator: "\n")
        try content.write(to: fileURL, atomically: true, encoding: .utf8)

        let result = try await GitRepositoryService().patchAndCompare(
            repoPath: directory.path,
            filePath: fileName,
            lineLimit: 100,
            hints: GitRepositoryService.DiffHints(hasStaged: false, hasUnstaged: false, isUntrackedOrNew: true)
        )

        #expect(result.additions == 100)
        #expect(result.truncated)
        #expect(result.rows.count == 101)
        #expect(result.rows.last?.newLineNumber == 100)
    }

    @Test("staged new file diff reads index content")
    func stagedNewFileDiffReadsIndexContent() async throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }

        _ = try await GitProcessRunner.runGit(repoPath: directory.path, arguments: ["init"])
        let fileName = "new.txt"
        let fileURL = directory.appendingPathComponent(fileName)
        try "staged\n".write(to: fileURL, atomically: true, encoding: .utf8)
        _ = try await GitProcessRunner.runGit(repoPath: directory.path, arguments: ["add", fileName])
        try "unstaged\n".write(to: fileURL, atomically: true, encoding: .utf8)

        let result = try await GitRepositoryService().patchAndCompare(
            repoPath: directory.path,
            filePath: fileName,
            lineLimit: nil,
            hints: GitRepositoryService.DiffHints(hasStaged: true, hasUnstaged: false, isUntrackedOrNew: false)
        )

        #expect(result.rows.contains { $0.newText == "staged" })
        #expect(!result.rows.contains { $0.newText == "unstaged" })
    }

    @Test("untracked symlink outside repository is rejected")
    func untrackedSymlinkOutsideRepositoryIsRejected() async throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let outsideDirectory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: outsideDirectory, withIntermediateDirectories: true)
        defer {
            try? FileManager.default.removeItem(at: directory)
            try? FileManager.default.removeItem(at: outsideDirectory)
        }

        let outsideFile = outsideDirectory.appendingPathComponent("secret.txt")
        try "secret\n".write(to: outsideFile, atomically: true, encoding: .utf8)
        let symlink = directory.appendingPathComponent("linked.txt")
        try FileManager.default.createSymbolicLink(at: symlink, withDestinationURL: outsideFile)

        do {
            _ = try await GitRepositoryService().patchAndCompare(
                repoPath: directory.path,
                filePath: "linked.txt",
                lineLimit: nil,
                hints: GitRepositoryService.DiffHints(hasStaged: false, hasUnstaged: false, isUntrackedOrNew: true)
            )
            Issue.record("Expected outside repository symlink to be rejected.")
        } catch let error as GitRepositoryService.GitError {
            #expect(error.errorDescription == "File path is outside the repository.")
        }
    }

    @Test("pull request diff ref is namespaced by number")
    func pullRequestDiffRefIsNamespacedByNumber() {
        #expect(GitRepositoryService.localPullRequestDiffRef(number: 535) == "refs/muxy/pull/535/head")
    }

    @Test("pull request diff base ref is namespaced by number")
    func pullRequestDiffBaseRefIsNamespacedByNumber() {
        #expect(GitRepositoryService.localPullRequestDiffBaseRef(number: 535) == "refs/muxy/pull/535/base")
    }

    @Test("pull request diff refs fetch the base branch alongside the head")
    func pullRequestDiffRefsFetchBaseBranch() async throws {
        let fixture = try await PullRequestDiffFixture(baseBranch: "release+beta")
        defer { fixture.cleanUp() }

        let refs = try await GitRepositoryService().fetchPullRequestDiffRefs(
            repoPath: fixture.clone.path,
            number: 10,
            remote: "origin",
            baseBranch: "release+beta"
        )

        #expect(refs.head == "refs/muxy/pull/10/head")
        #expect(refs.base == "refs/muxy/pull/10/base")
    }

    @Test("pull request diff refs fall back to the head when the base branch is missing")
    func pullRequestDiffRefsFallBackWhenBaseBranchMissing() async throws {
        let fixture = try await PullRequestDiffFixture()
        defer { fixture.cleanUp() }

        let refs = try await GitRepositoryService().fetchPullRequestDiffRefs(
            repoPath: fixture.clone.path,
            number: 10,
            remote: "origin",
            baseBranch: "deleted-base"
        )

        #expect(refs.head == "refs/muxy/pull/10/head")
        #expect(refs.base == nil)
    }

    @Test("pull request merge base resolves against the base branch while on the head branch")
    func pullRequestMergeBaseResolvesAgainstBaseBranch() async throws {
        let fixture = try await PullRequestDiffFixture()
        defer { fixture.cleanUp() }

        let refs = try await GitRepositoryService().fetchPullRequestDiffRefs(
            repoPath: fixture.clone.path,
            number: 10,
            remote: "origin",
            baseBranch: "main"
        )
        _ = try await GitProcessRunner.runGit(
            repoPath: fixture.clone.path,
            arguments: ["checkout", "--detach", refs.head]
        )

        let againstBase = try await fixture.mergeBase(refs.base ?? "HEAD", refs.head)
        let againstHead = try await fixture.mergeBase("HEAD", refs.head)

        #expect(againstBase == fixture.baseCommit)
        #expect(againstHead != fixture.baseCommit)

        let diff = try await GitProcessRunner.runGit(
            repoPath: fixture.clone.path,
            arguments: ["diff", "--name-only", "\(againstBase)...\(refs.head)"]
        )
        #expect(diff.stdout.contains("feature.txt"))
    }

    @Test("github remote name resolves matching owner repository")
    func githubRemoteNameResolvesMatchingOwnerRepository() {
        let remotes = """
        upstream\tgit@github.com:owner/repo.git (fetch)
        upstream\tgit@github.com:owner/repo.git (push)
        origin\tgit@github.com:fork/repo.git (fetch)
        origin\tgit@github.com:fork/repo.git (push)
        """

        #expect(GitRepositoryService.githubRemoteName(fromRemoteList: remotes, nameWithOwner: "owner/repo") == "upstream")
    }
}

private struct PullRequestDiffFixture {
    let origin: URL
    let clone: URL
    let baseCommit: String

    init(baseBranch: String = "main") async throws {
        origin = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        clone = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: origin, withIntermediateDirectories: true)

        try await Self.git(origin, ["init", "--initial-branch=\(baseBranch)"])
        try await Self.git(origin, ["config", "user.email", "test@example.com"])
        try await Self.git(origin, ["config", "user.name", "Test"])
        try "base\n".write(to: origin.appendingPathComponent("base.txt"), atomically: true, encoding: .utf8)
        try await Self.git(origin, ["add", "."])
        try await Self.git(origin, ["commit", "-m", "base"])

        let baseResult = try await GitProcessRunner.runGit(repoPath: origin.path, arguments: ["rev-parse", "HEAD"])
        baseCommit = baseResult.stdout.trimmingCharacters(in: .whitespacesAndNewlines)

        try await Self.git(origin, ["checkout", "-b", "feature"])
        try "feature\n".write(to: origin.appendingPathComponent("feature.txt"), atomically: true, encoding: .utf8)
        try await Self.git(origin, ["add", "."])
        try await Self.git(origin, ["commit", "-m", "feature"])
        try await Self.git(origin, ["update-ref", "refs/pull/10/head", "refs/heads/feature"])

        try await Self.git(origin, ["checkout", baseBranch])
        try "advanced\n".write(to: origin.appendingPathComponent("advanced.txt"), atomically: true, encoding: .utf8)
        try await Self.git(origin, ["add", "."])
        try await Self.git(origin, ["commit", "-m", "advance main"])

        try await Self.git(FileManager.default.temporaryDirectory, ["clone", origin.path, clone.path])
    }

    func mergeBase(_ lhs: String, _ rhs: String) async throws -> String {
        let result = try await GitProcessRunner.runGit(repoPath: clone.path, arguments: ["merge-base", lhs, rhs])
        return result.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    func cleanUp() {
        try? FileManager.default.removeItem(at: origin)
        try? FileManager.default.removeItem(at: clone)
    }

    private static func git(_ directory: URL, _ arguments: [String]) async throws {
        _ = try await GitProcessRunner.runGit(repoPath: directory.path, arguments: arguments)
    }
}
