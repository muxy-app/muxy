import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionStore start ordering")
@MainActor
struct ExtensionStoreStartOrderingTests {
    @Test("publishes the token snapshot before the host process is spawned")
    func publishesTokenBeforeSpawn() throws {
        let directory = try makeBackgroundExtension(name: "ordered-ext")
        defer { try? FileManager.default.removeItem(at: directory.parent) }

        let recorder = SnapshotOrderRecorder()
        let store = ExtensionStore.makeForTesting(
            rootDirectory: directory.parent,
            snapshotSink: recorder,
            resolveHostURL: { URL(fileURLWithPath: "/usr/bin/true") }
        )
        recorder.bind(store: store, extensionID: "ordered-ext")

        store.startAll()

        let publishWithToken = try #require(
            recorder.records.first(where: { $0.hasToken }),
            "the token must be published to the socket server at least once"
        )
        #expect(
            publishWithToken.processSpawned == false,
            "the token snapshot must reach the server before the host process is spawned"
        )
    }

    @Test("removes the token from the snapshot when the host fails to spawn")
    func removesTokenWhenSpawnFails() throws {
        let directory = try makeBackgroundExtension(name: "failing-ext")
        defer { try? FileManager.default.removeItem(at: directory.parent) }

        let recorder = SnapshotOrderRecorder()
        let store = ExtensionStore.makeForTesting(
            rootDirectory: directory.parent,
            snapshotSink: recorder,
            resolveHostURL: { URL(fileURLWithPath: "/nonexistent/muxy-host-binary") }
        )
        recorder.bind(store: store, extensionID: "failing-ext")

        store.startAll()

        #expect(recorder.records.contains(where: { $0.hasToken }))
        let last = try #require(recorder.records.last)
        #expect(last.hasToken == false, "a failed spawn must clear the token from the published snapshot")
    }

    private func makeBackgroundExtension(name: String) throws -> (url: URL, parent: URL) {
        let parent = FileManager.default.temporaryDirectory.appendingPathComponent("exts-\(UUID().uuidString)")
        let directory = parent.appendingPathComponent(name)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let manifest = """
        {
            "name": "\(name)",
            "version": "1.0.0",
            "background": "background.js",
            "events": ["pane.created"]
        }
        """
        try Data(manifest.utf8).write(to: directory.appendingPathComponent("manifest.json"))
        try Data("console.log('hi')\n".utf8).write(to: directory.appendingPathComponent("background.js"))
        return (directory, parent)
    }
}

private extension URL {
    var parent: URL { deletingLastPathComponent() }
}

@MainActor
private final class SnapshotOrderRecorder: ExtensionSnapshotSink {
    struct Record {
        let hasToken: Bool
        let processSpawned: Bool
    }

    private(set) var records: [Record] = []
    private weak var store: ExtensionStore?
    private var extensionID = ""

    func bind(store: ExtensionStore, extensionID: String) {
        self.store = store
        self.extensionID = extensionID
    }

    nonisolated func applyExtensionSnapshot(_ snapshot: NotificationSocketServer.ExtensionSnapshot) {
        MainActor.assumeIsolated {
            let hasToken = snapshot.entries[extensionID] != nil
            let spawned = store?.hasSpawnedProcessForTesting(extensionID: extensionID) ?? false
            records.append(Record(hasToken: hasToken, processSpawned: spawned))
        }
    }
}
