import CoreServices
import Foundation
import Testing

@testable import Muxy

@Suite("GitWorktreesWatcher")
struct GitWorktreesWatcherTests {
    private func makeTempRepo() -> URL {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("GitWorktreesWatcherTests-\(UUID().uuidString)", isDirectory: true)
        let gitDir = dir.appendingPathComponent(".git", isDirectory: true)
        try? FileManager.default.createDirectory(at: gitDir, withIntermediateDirectories: true)
        return dir
    }

    private func flags(_ values: Int...) -> FSEventStreamEventFlags {
        FSEventStreamEventFlags(values.reduce(0, |))
    }

    @Test("structural change requires a directory event under worktrees")
    func structuralChangeFilter() {
        let created = flags(kFSEventStreamEventFlagItemIsDir, kFSEventStreamEventFlagItemCreated)
        let removed = flags(kFSEventStreamEventFlagItemIsDir, kFSEventStreamEventFlagItemRemoved)
        let renamed = flags(kFSEventStreamEventFlagItemIsDir, kFSEventStreamEventFlagItemRenamed)

        #expect(GitWorktreesWatcher.isWorktreeStructuralChange(
            path: "/repo/.git/worktrees/feature", flag: created
        ))
        #expect(GitWorktreesWatcher.isWorktreeStructuralChange(
            path: "/repo/.git/worktrees/feature", flag: removed
        ))
        #expect(GitWorktreesWatcher.isWorktreeStructuralChange(
            path: "/repo/.git/worktrees/feature", flag: renamed
        ))
    }

    @Test("ignores file writes inside worktree admin dir")
    func ignoresFileWrites() {
        let fileWrite = flags(kFSEventStreamEventFlagItemCreated, kFSEventStreamEventFlagItemModified)
        #expect(!GitWorktreesWatcher.isWorktreeStructuralChange(
            path: "/repo/.git/worktrees/feature/HEAD", flag: fileWrite
        ))
    }

    @Test("ignores directory modifications that are not create/remove/rename")
    func ignoresNonStructuralDirEvents() {
        let modified = flags(kFSEventStreamEventFlagItemIsDir, kFSEventStreamEventFlagItemModified)
        #expect(!GitWorktreesWatcher.isWorktreeStructuralChange(
            path: "/repo/.git/worktrees/feature", flag: modified
        ))
    }

    @Test("ignores directory events outside the worktrees path")
    func ignoresOtherGitDirs() {
        let created = flags(kFSEventStreamEventFlagItemIsDir, kFSEventStreamEventFlagItemCreated)
        #expect(!GitWorktreesWatcher.isWorktreeStructuralChange(
            path: "/repo/.git/refs/heads/feature", flag: created
        ))
    }

    @Test("treats HEAD changes as relevant for primary and linked worktrees")
    func detectsHeadReferenceChanges() {
        #expect(GitWorktreesWatcher.isHeadReferenceChange(path: "/repo/.git/HEAD"))
        #expect(GitWorktreesWatcher.isHeadReferenceChange(path: "/repo/.git/worktrees/feature/HEAD"))
    }

    @Test("ignores reflog and lock writes so commits do not trigger refreshes")
    func ignoresReflogAndLockWrites() {
        #expect(!GitWorktreesWatcher.isHeadReferenceChange(path: "/repo/.git/logs/HEAD"))
        #expect(!GitWorktreesWatcher.isHeadReferenceChange(path: "/repo/.git/worktrees/feature/logs/HEAD"))
        #expect(!GitWorktreesWatcher.isHeadReferenceChange(path: "/repo/.git/HEAD.lock"))
    }

    @Test("resolves the .git directory for a primary repo")
    func resolvesPrimaryGitDirectory() {
        let repo = makeTempRepo()
        defer { try? FileManager.default.removeItem(at: repo) }

        let resolved = GitWorktreesWatcher.resolveGitDirectory(forRepoPath: repo.path)
        #expect(resolved == repo.appendingPathComponent(".git").path)
    }

    @Test("resolves the common .git directory when .git is a linked-worktree gitfile")
    func resolvesLinkedWorktreeGitDirectory() {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("GitWorktreesWatcherTests-\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }

        let gitFile = dir.appendingPathComponent(".git")
        try? "gitdir: /main/checkout/.git/worktrees/feature\n".data(using: .utf8)?.write(to: gitFile)

        let resolved = GitWorktreesWatcher.resolveGitDirectory(forRepoPath: dir.path)
        #expect(resolved == "/main/checkout/.git")
    }

    @Test("resolves relative linked-worktree gitfile targets")
    func resolvesRelativeLinkedWorktreeGitDirectory() {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("GitWorktreesWatcherTests-\(UUID().uuidString)", isDirectory: true)
        let mainGit = root.appendingPathComponent("main/.git", isDirectory: true)
        let worktree = root.appendingPathComponent("feature", isDirectory: true)
        try? FileManager.default.createDirectory(at: mainGit, withIntermediateDirectories: true)
        try? FileManager.default.createDirectory(at: worktree, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }

        let gitFile = worktree.appendingPathComponent(".git")
        try? "gitdir: ../main/.git/worktrees/feature\n".data(using: .utf8)?.write(to: gitFile)

        let resolved = GitWorktreesWatcher.resolveGitDirectory(forRepoPath: worktree.path)
        #expect(resolved == mainGit.path)
    }

    @Test("returns nil when there is no git directory")
    func returnsNilForNonRepo() {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("GitWorktreesWatcherTests-\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }

        #expect(GitWorktreesWatcher.resolveGitDirectory(forRepoPath: dir.path) == nil)
    }

    @Test("fires when a worktree admin directory is created")
    func firesOnWorktreeDirectoryCreation() async throws {
        let repo = makeTempRepo()
        defer { try? FileManager.default.removeItem(at: repo) }

        let counter = FireCounter()
        let watcher = GitWorktreesWatcher(repoPath: repo.path) { counter.increment() }
        #expect(watcher != nil)

        try await Task.sleep(nanoseconds: 400_000_000)
        let worktreeDir = repo.appendingPathComponent(".git/worktrees/feature", isDirectory: true)
        try FileManager.default.createDirectory(at: worktreeDir, withIntermediateDirectories: true)

        let fired = await waitFor(timeout: 5.0) { counter.value > 0 }
        #expect(fired)
        _ = watcher
    }

    @Test("fires when HEAD changes (branch switch or rename)")
    func firesOnHeadChange() async throws {
        let repo = makeTempRepo()
        defer { try? FileManager.default.removeItem(at: repo) }

        let head = repo.appendingPathComponent(".git/HEAD")
        try "ref: refs/heads/old\n".data(using: .utf8)!.write(to: head)

        let counter = FireCounter()
        let watcher = GitWorktreesWatcher(repoPath: repo.path) { counter.increment() }
        #expect(watcher != nil)

        try await Task.sleep(nanoseconds: 400_000_000)
        try "ref: refs/heads/new\n".data(using: .utf8)!.write(to: head)

        let fired = await waitFor(timeout: 5.0) { counter.value > 0 }
        #expect(fired)
        _ = watcher
    }

    private func waitFor(timeout: TimeInterval, condition: () -> Bool) async -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if condition() { return true }
            try? await Task.sleep(nanoseconds: 50_000_000)
        }
        return condition()
    }
}

private final class FireCounter: @unchecked Sendable {
    private let lock = NSLock()
    private var count = 0

    func increment() {
        lock.lock()
        count += 1
        lock.unlock()
    }

    var value: Int {
        lock.lock()
        defer { lock.unlock() }
        return count
    }
}
