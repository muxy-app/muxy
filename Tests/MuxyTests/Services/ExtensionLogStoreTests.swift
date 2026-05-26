import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionLogStore")
struct ExtensionLogStoreTests {
    @Test("append writes the line to the extension log file")
    func appendWritesLine() async throws {
        let store = ExtensionLogStore.shared
        let directory = try makeExtensionDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let extensionID = directory.lastPathComponent

        store.register(extensionID: extensionID, directory: directory)
        store.append(extensionID: extensionID, line: "hello world")
        try await waitForCondition(timeout: 1.0) {
            FileManager.default.fileExists(atPath: store.logURL(extensionID: extensionID, directory: directory).path)
        }
        store.unregister(extensionID: extensionID)

        let logURL = store.logURL(extensionID: extensionID, directory: directory)
        let text = try String(contentsOf: logURL, encoding: .utf8)
        #expect(text.contains("hello world"))
    }

    @Test("clear empties the log file")
    func clearEmptiesLog() async throws {
        let store = ExtensionLogStore.shared
        let directory = try makeExtensionDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let extensionID = directory.lastPathComponent

        store.register(extensionID: extensionID, directory: directory)
        store.append(extensionID: extensionID, line: "first")
        store.append(extensionID: extensionID, line: "second")
        let logURL = store.logURL(extensionID: extensionID, directory: directory)
        try await waitForCondition(timeout: 1.0) {
            (try? String(contentsOf: logURL, encoding: .utf8))?.contains("second") == true
        }

        store.clear(extensionID: extensionID)
        try await waitForCondition(timeout: 1.0) {
            (try? Data(contentsOf: logURL).isEmpty) == true
        }
        store.unregister(extensionID: extensionID)
        let data = try Data(contentsOf: logURL)
        #expect(data.isEmpty)
    }

    private func makeExtensionDirectory() throws -> URL {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("ext-log-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }

    private func waitForCondition(
        timeout: TimeInterval,
        check: @escaping () -> Bool
    ) async throws {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if check() { return }
            try await Task.sleep(nanoseconds: 30_000_000)
        }
        Issue.record("condition not satisfied within \(timeout)s")
    }
}
