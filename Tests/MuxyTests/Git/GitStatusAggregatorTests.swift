import Foundation
import Testing

@testable import Muxy

@Suite("GitStatusAggregator")
struct GitStatusAggregatorTests {
    @Test("snapshot reports branch, staged and unstaged files")
    func snapshotReportsFiles() async throws {
        let repo = try TempAggregatorRepo()
        defer { repo.cleanup() }

        try repo.commit(file: "a.txt", contents: "1\n", message: "base")
        try repo.write(file: "a.txt", contents: "changed\n")
        try repo.write(file: "staged.txt", contents: "new\n")
        try repo.run("add", "staged.txt")

        let snapshot = try await GitStatusAggregator.snapshot(repoPath: repo.path, includePullRequest: false)

        #expect(snapshot.branch == "main")
        #expect(snapshot.branches.contains("main"))
        #expect(snapshot.stagedFiles.contains { $0.path == "staged.txt" })
        #expect(snapshot.unstagedFiles.contains { $0.path == "a.txt" })
        #expect(snapshot.pullRequest == nil)
    }

    @Test("clean repo has no changed files")
    func cleanRepoHasNoFiles() async throws {
        let repo = try TempAggregatorRepo()
        defer { repo.cleanup() }

        try repo.commit(file: "a.txt", contents: "1\n", message: "base")

        let snapshot = try await GitStatusAggregator.snapshot(repoPath: repo.path, includePullRequest: false)

        #expect(snapshot.files.isEmpty)
        #expect(snapshot.aheadBehind.hasUpstream == false)
    }
}

private struct TempAggregatorRepo {
    let path: String
    private let parent: String

    init() throws {
        let base = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-aggregator-tests-\(UUID().uuidString)", isDirectory: true)
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

    func write(file: String, contents: String) throws {
        let fileURL = URL(fileURLWithPath: path).appendingPathComponent(file)
        try contents.write(to: fileURL, atomically: true, encoding: .utf8)
    }

    func commit(file: String, contents: String, message: String) throws {
        try write(file: file, contents: contents)
        try run("add", file)
        try run("commit", "-q", "-m", message)
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
        if process.terminationStatus != 0 {
            let output = String(data: data, encoding: .utf8) ?? ""
            throw NSError(
                domain: "GitTestRepo",
                code: Int(process.terminationStatus),
                userInfo: [NSLocalizedDescriptionKey: output]
            )
        }
    }
}
