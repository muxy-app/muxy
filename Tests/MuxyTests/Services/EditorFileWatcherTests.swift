import Foundation
import Testing

@testable import Muxy

@Suite("EditorFileWatcher")
struct EditorFileWatcherTests {
    private func makeTempFile(contents: String = "initial") async -> URL {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("EditorFileWatcherTests-\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let url = dir.appendingPathComponent("file.txt")
        try? contents.data(using: .utf8)?.write(to: url)
        try? await Task.sleep(nanoseconds: 500_000_000)
        return url
    }

    @Test("init returns nil for empty path")
    func initRejectsEmptyPath() {
        let watcher = EditorFileWatcher(filePath: "") {}
        #expect(watcher == nil)
    }

    @Test("fires handler when watched file changes")
    func firesOnChange() async throws {
        let url = await makeTempFile()
        let counter = FireCounter()
        let watcher = EditorFileWatcher(filePath: url.path, debounceInterval: 0.1) {
            counter.increment()
        }
        #expect(watcher != nil)

        try await Task.sleep(nanoseconds: 250_000_000)
        try "updated".data(using: .utf8)!.write(to: url)

        let fired = await waitFor(timeout: 5.0) { counter.value > 0 }
        #expect(fired)
        _ = watcher
    }

    @Test("ignores changes to sibling files in the same directory")
    func ignoresSiblingChanges() async throws {
        let url = await makeTempFile()
        let sibling = url.deletingLastPathComponent().appendingPathComponent("other.txt")
        let counter = FireCounter()
        let watcher = EditorFileWatcher(filePath: url.path, debounceInterval: 0.1) {
            counter.increment()
        }
        #expect(watcher != nil)

        try await Task.sleep(nanoseconds: 250_000_000)
        try "sibling".data(using: .utf8)!.write(to: sibling)
        try await Task.sleep(nanoseconds: 1_000_000_000)

        #expect(counter.value == 0)
        _ = watcher
    }

    @Test("debounces rapid successive writes into a single fire")
    func debouncesRapidWrites() async throws {
        let url = await makeTempFile()
        let counter = FireCounter()
        let watcher = EditorFileWatcher(filePath: url.path, debounceInterval: 0.4) {
            counter.increment()
        }
        #expect(watcher != nil)

        try await Task.sleep(nanoseconds: 250_000_000)
        for index in 0 ..< 5 {
            try "v\(index)".data(using: .utf8)!.write(to: url)
            try await Task.sleep(nanoseconds: 50_000_000)
        }
        try await Task.sleep(nanoseconds: 1_500_000_000)

        #expect(counter.value == 1)
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
