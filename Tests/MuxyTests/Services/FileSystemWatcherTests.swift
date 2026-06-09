import Foundation
import Testing

@testable import Muxy

@Suite("FileSystemWatcher")
struct FileSystemWatcherTests {
    private func makeTempDir() async -> URL {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("FileSystemWatcherTests-\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        try? await Task.sleep(nanoseconds: 500_000_000)
        return dir
    }

    @Test("init returns nil for a missing directory")
    func initRejectsMissingDirectory() {
        let watcher = FileSystemWatcher(directoryPath: "/this/path/does/not/exist") { _ in }
        #expect(watcher == nil)
    }

    @Test("delivers changed file paths to the handler")
    func deliversChangedPaths() async throws {
        let dir = await makeTempDir()
        let collector = PathCollector()
        let watcher = FileSystemWatcher(directoryPath: dir.path) { paths in
            collector.add(paths)
        }
        #expect(watcher != nil)

        try await Task.sleep(nanoseconds: 250_000_000)
        let target = dir.appendingPathComponent("note.txt")
        try "hello".data(using: .utf8)!.write(to: target)

        let fired = await waitFor(timeout: 5.0) { collector.contains(target.path) }
        #expect(fired)
        _ = watcher
    }

    @Test("skips Git internal lock-file noise")
    func skipsGitLockNoise() async throws {
        let dir = await makeTempDir()
        let gitDir = dir.appendingPathComponent(".git", isDirectory: true)
        try? FileManager.default.createDirectory(at: gitDir, withIntermediateDirectories: true)
        try await Task.sleep(nanoseconds: 500_000_000)

        let collector = PathCollector()
        let watcher = FileSystemWatcher(directoryPath: dir.path) { paths in
            collector.add(paths)
        }
        #expect(watcher != nil)

        try await Task.sleep(nanoseconds: 500_000_000)
        let lock = gitDir.appendingPathComponent("index.lock")
        try "lock".data(using: .utf8)!.write(to: lock)
        try await Task.sleep(nanoseconds: 1_000_000_000)

        #expect(collector.count == 0)
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

private final class PathCollector: @unchecked Sendable {
    private let lock = NSLock()
    private var paths: [String] = []

    func add(_ values: [String]) {
        lock.lock()
        paths.append(contentsOf: values)
        lock.unlock()
    }

    func contains(_ value: String) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        let resolved = (value as NSString).resolvingSymlinksInPath
        return paths.contains { ($0 as NSString).resolvingSymlinksInPath == resolved }
    }

    var count: Int {
        lock.lock()
        defer { lock.unlock() }
        return paths.count
    }
}
