import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionLogStore", .serialized)
struct ExtensionLogStoreTests {
    @Test("append writes the line to the extension log file")
    func appendWritesLine() throws {
        let store = ExtensionLogStore.shared
        let directory = try makeExtensionDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let extensionID = directory.lastPathComponent

        store.register(extensionID: extensionID, directory: directory)
        store.append(extensionID: extensionID, line: "hello world")
        store.flush()
        store.unregister(extensionID: extensionID)
        store.flush()

        let logURL = store.logURL(extensionID: extensionID, directory: directory)
        let text = try String(contentsOf: logURL, encoding: .utf8)
        #expect(text.contains("hello world"))
    }

    @Test("clear empties the log file")
    func clearEmptiesLog() throws {
        let store = ExtensionLogStore.shared
        let directory = try makeExtensionDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let extensionID = directory.lastPathComponent

        store.register(extensionID: extensionID, directory: directory)
        store.append(extensionID: extensionID, line: "first")
        store.append(extensionID: extensionID, line: "second")
        store.flush()

        store.clear(extensionID: extensionID)
        store.flush()
        store.unregister(extensionID: extensionID)
        store.flush()

        let logURL = store.logURL(extensionID: extensionID, directory: directory)
        let data = try Data(contentsOf: logURL)
        #expect(data.isEmpty)
    }

    private func makeExtensionDirectory() throws -> URL {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("ext-log-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }
}
