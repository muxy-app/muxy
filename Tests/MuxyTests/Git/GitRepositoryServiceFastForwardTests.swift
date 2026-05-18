import Foundation
import Testing

@testable import Muxy

@Suite("GitRepositoryService.fastForwardBranch")
struct GitRepositoryServiceFastForwardTests {
    @Test("inactive branch fast-forward refreshes remote tracking ref")
    func inactiveBranchRefreshesRemoteTrackingRef() async throws {
        let repo = try TempRemoteGitRepo()
        defer { repo.cleanup() }

        try repo.commit(file: "main.txt", contents: "1", message: "main")
        try repo.run("push", "-u", "origin", "main")
        try repo.run("checkout", "-b", "release")
        try repo.commit(file: "release.txt", contents: "1", message: "release")
        try repo.run("push", "-u", "origin", "release")
        try repo.run("checkout", "main")

        try repo.cloneToUpstream()
        try repo.runUpstream("checkout", "release")
        try repo.commitUpstream(file: "release.txt", contents: "2", message: "advance release")
        try repo.runUpstream("push", "origin", "release")

        let succeeded = await GitRepositoryService().fastForwardBranch(repoPath: repo.path, branch: "release")

        #expect(succeeded)
        let localRelease = try repo.runCapturing("rev-parse", "release")
        let trackingRelease = try repo.runCapturing("rev-parse", "origin/release")
        #expect(localRelease == trackingRelease)
    }
}

private struct TempRemoteGitRepo {
    let path: String
    private let parent: String
    private let remotePath: String
    private let upstreamPath: String

    init() throws {
        let base = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-git-service-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: base, withIntermediateDirectories: true)
        parent = base.path
        remotePath = base.appendingPathComponent("remote.git", isDirectory: true).path
        path = base.appendingPathComponent("repo", isDirectory: true).path
        upstreamPath = base.appendingPathComponent("upstream", isDirectory: true).path
        try Self.runGit(at: parent, args: ["init", "-q", "--bare", "-b", "main", remotePath])
        try Self.runGit(at: parent, args: ["clone", "-q", remotePath, path])
        try configureRepository(at: path)
    }

    func cleanup() {
        try? FileManager.default.removeItem(atPath: parent)
    }

    func cloneToUpstream() throws {
        try Self.runGit(at: parent, args: ["clone", "-q", remotePath, upstreamPath])
        try configureRepository(at: upstreamPath)
    }

    func commit(file: String, contents: String, message: String) throws {
        try commit(at: path, file: file, contents: contents, message: message)
    }

    func commitUpstream(file: String, contents: String, message: String) throws {
        try commit(at: upstreamPath, file: file, contents: contents, message: message)
    }

    func run(_ args: String...) throws {
        try Self.runGit(at: path, args: args)
    }

    func runUpstream(_ args: String...) throws {
        try Self.runGit(at: upstreamPath, args: args)
    }

    func runCapturing(_ args: String...) throws -> String {
        try Self.runGit(at: path, args: args)
    }

    private func configureRepository(at path: String) throws {
        try Self.runGit(at: path, args: ["config", "user.email", "test@example.com"])
        try Self.runGit(at: path, args: ["config", "user.name", "Test"])
        try Self.runGit(at: path, args: ["config", "commit.gpgsign", "false"])
    }

    private func commit(at path: String, file: String, contents: String, message: String) throws {
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
