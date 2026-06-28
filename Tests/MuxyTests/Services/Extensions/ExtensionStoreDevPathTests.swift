import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionStore dev path loading")
@MainActor
struct ExtensionStoreDevPathTests {
    @Test("loads a dev extension whose folder name differs from its name")
    func loadsDevWithMismatchedFolder() throws {
        let root = try makeRoot()
        let devParent = try makeRoot()
        defer {
            try? FileManager.default.removeItem(at: root)
            try? FileManager.default.removeItem(at: devParent)
        }
        let devDir = try makeExtension(name: "real-name", directoryName: "my-ext-fork", in: devParent)

        let store = makeStore(root: root, devPaths: [devDir.path])
        store.startAll()

        let status = try #require(store.statuses.first { $0.id == "real-name" })
        #expect(status.isDev)
        #expect(status.devSourcePath == devDir.path)
        #expect(store.loadFailures.isEmpty)
    }

    @Test("a dev path colliding with an installed extension surfaces a failure")
    func collisionSurfacesFailure() throws {
        let root = try makeRoot()
        let devParent = try makeRoot()
        defer {
            try? FileManager.default.removeItem(at: root)
            try? FileManager.default.removeItem(at: devParent)
        }
        _ = try makeExtension(name: "shared", directoryName: "shared", in: root)
        let devDir = try makeExtension(name: "shared", directoryName: "shared-dev", in: devParent)

        let store = makeStore(root: root, devPaths: [devDir.path])
        store.startAll()

        #expect(store.statuses.filter { $0.id == "shared" }.count == 1)
        #expect(store.statuses.first { $0.id == "shared" }?.isDev == false)
        #expect(!store.loadFailures.isEmpty)
    }

    @Test("a missing dev path fails without aborting the rest of the scan")
    func missingDevPathDoesNotAbort() throws {
        let root = try makeRoot()
        let devParent = try makeRoot()
        defer {
            try? FileManager.default.removeItem(at: root)
            try? FileManager.default.removeItem(at: devParent)
        }
        _ = try makeExtension(name: "installed", directoryName: "installed", in: root)
        let missing = devParent.appendingPathComponent("does-not-exist")

        let store = makeStore(root: root, devPaths: [missing.path])
        store.startAll()

        #expect(store.statuses.contains { $0.id == "installed" })
        #expect(!store.loadFailures.isEmpty)
    }

    @Test("a failed dev path load carries its source path so it can be removed")
    func failedDevPathLoadCarriesSourcePath() throws {
        let root = try makeRoot()
        let devParent = try makeRoot()
        defer {
            try? FileManager.default.removeItem(at: root)
            try? FileManager.default.removeItem(at: devParent)
        }
        let missing = devParent.appendingPathComponent("does-not-exist")

        let store = makeStore(root: root, devPaths: [missing.path])
        store.startAll()

        let failure = try #require(store.loadFailures.first)
        #expect(failure.devSourcePath == missing.path)
    }

    @Test("a failed root extension load is not removable as a dev path")
    func failedRootLoadHasNoDevSourcePath() throws {
        let root = try makeRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let broken = root.appendingPathComponent("broken")
        try FileManager.default.createDirectory(at: broken, withIntermediateDirectories: true)

        let store = makeStore(root: root, devPaths: [])
        store.startAll()

        let failure = try #require(store.loadFailures.first)
        #expect(failure.devSourcePath == nil)
    }

    private func makeRoot() throws -> URL {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("exts-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        return root
    }

    @discardableResult
    private func makeExtension(name: String, directoryName: String, in root: URL) throws -> URL {
        let directory = root.appendingPathComponent(directoryName)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let manifest = """
        {
            "name": "\(name)",
            "version": "1.0.0"
        }
        """
        try ExtensionManifestFixture.write(flatManifest: manifest, to: directory)
        return directory
    }

    private func makeStore(root: URL, devPaths: [String]) -> ExtensionStore {
        ExtensionStore.makeForTesting(
            rootDirectory: root,
            snapshotSink: NoopDevPathSnapshotSink(),
            resolveHostURL: { nil },
            devPathsProvider: { devPaths }
        )
    }
}

@MainActor
private final class NoopDevPathSnapshotSink: ExtensionSnapshotSink {
    nonisolated func applyExtensionSnapshot(_: NotificationSocketServer.ExtensionSnapshot) {}
}
