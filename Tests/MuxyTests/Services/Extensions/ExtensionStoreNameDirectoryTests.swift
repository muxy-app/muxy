import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionStore name/directory enforcement")
@MainActor
struct ExtensionStoreNameDirectoryTests {
    @Test("loads an extension whose directory matches its name")
    func loadsWhenDirectoryMatchesName() throws {
        let root = try makeRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        try makeExtension(name: "matching-ext", directoryName: "matching-ext", in: root)

        let store = makeStore(root: root)
        store.startAll()

        #expect(store.statuses.contains { $0.id == "matching-ext" })
        #expect(store.loadFailures.isEmpty)
    }

    @Test("rejects an extension whose directory differs from its name")
    func rejectsWhenDirectoryDiffersFromName() throws {
        let root = try makeRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        try makeExtension(name: "real-name", directoryName: "other-dir", in: root)

        let store = makeStore(root: root)
        store.startAll()

        #expect(store.statuses.isEmpty)
        let failure = try #require(store.loadFailures.first)
        #expect(failure.message == ExtensionLoadError.nameDirectoryMismatch(
            name: "real-name",
            directory: "other-dir"
        ).localizedDescription)
    }

    private func makeRoot() throws -> URL {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("exts-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        return root
    }

    private func makeExtension(name: String, directoryName: String, in root: URL) throws {
        let directory = root.appendingPathComponent(directoryName)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let manifest = """
        {
            "name": "\(name)",
            "version": "1.0.0"
        }
        """
        try ExtensionManifestFixture.write(flatManifest: manifest, to: directory)
    }

    private func makeStore(root: URL) -> ExtensionStore {
        ExtensionStore.makeForTesting(
            rootDirectory: root,
            snapshotSink: NoopNameDirectorySnapshotSink(),
            resolveHostURL: { nil }
        )
    }
}

@MainActor
private final class NoopNameDirectorySnapshotSink: ExtensionSnapshotSink {
    nonisolated func applyExtensionSnapshot(_: NotificationSocketServer.ExtensionSnapshot) {}
}
