import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionStore manifest loading")
@MainActor
struct ExtensionStoreManifestLoadingTests {
    @Test("loadManifestsIfNeeded populates statuses from disk")
    func loadsManifestsEagerly() throws {
        let root = try makeRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        try makeExtension(name: "alpha", in: root)

        let store = makeStore(root: root)
        store.loadManifestsIfNeeded()

        #expect(store.hasLoadedFromDisk)
        #expect(store.statuses.contains { $0.id == "alpha" })
    }

    @Test("loadManifestsIfNeeded does not rescan once loaded")
    func skipsRescanWhenAlreadyLoaded() throws {
        let root = try makeRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        try makeExtension(name: "alpha", in: root)

        let store = makeStore(root: root)
        store.loadManifestsIfNeeded()
        try makeExtension(name: "beta", in: root)
        store.loadManifestsIfNeeded()

        #expect(!store.statuses.contains { $0.id == "beta" })
    }

    @Test("reload rescans disk after eager load")
    func reloadRescansAfterEagerLoad() throws {
        let root = try makeRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        try makeExtension(name: "alpha", in: root)

        let store = makeStore(root: root)
        store.loadManifestsIfNeeded()
        try makeExtension(name: "beta", in: root)
        store.reload()

        #expect(store.statuses.contains { $0.id == "beta" })
    }

    private func makeRoot() throws -> URL {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("exts-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        return root
    }

    private func makeExtension(name: String, in root: URL) throws {
        let directory = root.appendingPathComponent(name)
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
            snapshotSink: NoopManifestSnapshotSink(),
            resolveHostURL: { nil }
        )
    }
}

@MainActor
private final class NoopManifestSnapshotSink: ExtensionSnapshotSink {
    nonisolated func applyExtensionSnapshot(_: NotificationSocketServer.ExtensionSnapshot) {}
}
