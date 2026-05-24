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
