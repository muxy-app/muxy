import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionStore delete")
@MainActor
struct ExtensionStoreDeleteTests {
    @Test("removes the installed folder and drops the status")
    func deletesInstalledExtension() async throws {
        let root = makeRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = makeStore(root: root)

        try await store.install(expectedName: "demo-ext", zip: makeExtensionZip(name: "demo-ext"))
        let directory = root.appendingPathComponent("demo-ext")
        #expect(FileManager.default.fileExists(atPath: directory.path))

        try store.delete(extensionID: "demo-ext")

        #expect(!FileManager.default.fileExists(atPath: directory.path))
        #expect(!store.statuses.contains { $0.id == "demo-ext" })
    }

    @Test("purges enabled state, settings, and permission grants")
    func purgesPersistedState() async throws {
        let root = makeRoot()
        let extensionID = "delete-state-ext"
        defer {
            try? FileManager.default.removeItem(at: root)
            ExtensionEnabledStore.clear(extensionID: extensionID)
            ExtensionSettingsStore.shared.clearAll(extensionID: extensionID)
            ExtensionGrantStore.shared.removeAll(for: extensionID)
        }
        let store = makeStore(root: root)
        try await store.install(expectedName: extensionID, zip: makeExtensionZip(name: extensionID))

        ExtensionEnabledStore.setEnabled(true, extensionID: extensionID)
        ExtensionSettingsStore.shared.setValue(.string("dark"), extensionID: extensionID, key: "theme")
        ExtensionGrantStore.shared.add(ExtensionGrantRule(
            extensionID: extensionID,
            verb: .exec,
            match: .argvExact(["git", "status"]),
            decision: .allow
        ))

        try store.delete(extensionID: extensionID)

        #expect(!ExtensionEnabledStore.hasOverride(extensionID: extensionID))
        #expect(ExtensionSettingsStore.shared.value(extensionID: extensionID, key: "theme") == nil)
        #expect(ExtensionGrantStore.shared.rules(for: extensionID).isEmpty)
    }

    @Test("is a no-op for an unknown extension")
    func ignoresUnknownExtension() throws {
        let root = makeRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = makeStore(root: root)

        try store.delete(extensionID: "missing")

        #expect(store.statuses.isEmpty)
    }

    @Test("refuses to delete a dev extension and leaves its folder untouched")
    func rejectsDevExtension() throws {
        let root = makeRoot()
        let devParent = makeRoot()
        defer {
            try? FileManager.default.removeItem(at: root)
            try? FileManager.default.removeItem(at: devParent)
        }
        let devDir = try makeExtensionDirectory(name: "dev-ext", in: devParent)
        let store = makeStore(root: root, devPaths: [devDir.path])
        store.startAll()
        #expect(store.statuses.first { $0.id == "dev-ext" }?.isDev == true)

        #expect(throws: ExtensionDeleteError.devExtension) {
            try store.delete(extensionID: "dev-ext")
        }
        #expect(FileManager.default.fileExists(atPath: devDir.path))
        #expect(store.statuses.contains { $0.id == "dev-ext" })
    }

    private func makeRoot() -> URL {
        FileManager.default.temporaryDirectory.appendingPathComponent("delete-root-\(UUID().uuidString)")
    }

    private func makeStore(root: URL, devPaths: [String] = []) -> ExtensionStore {
        ExtensionStore.makeForTesting(
            rootDirectory: root,
            snapshotSink: NoopDeleteSnapshotSink(),
            resolveHostURL: { URL(fileURLWithPath: "/usr/bin/true") },
            devPathsProvider: { devPaths }
        )
    }

    @discardableResult
    private func makeExtensionDirectory(name: String, in parent: URL) throws -> URL {
        let directory = parent.appendingPathComponent(name)
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

    private func makeExtensionZip(name: String) throws -> Data {
        let fileManager = FileManager.default
        let workspace = fileManager.temporaryDirectory.appendingPathComponent("zip-src-\(UUID().uuidString)")
        let source = workspace.appendingPathComponent(name)
        try fileManager.createDirectory(at: source, withIntermediateDirectories: true)
        defer { try? fileManager.removeItem(at: workspace) }

        let manifest = """
        {
            "name": "\(name)",
            "version": "1.0.0",
            "background": "background.js"
        }
        """
        try ExtensionManifestFixture.write(flatManifest: manifest, to: source)
        try Data("console.log('hi')\n".utf8).write(to: source.appendingPathComponent("background.js"))

        let archive = workspace.appendingPathComponent("\(name).zip")
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/zip")
        process.arguments = ["-q", "-r", archive.path, name]
        process.currentDirectoryURL = workspace
        try process.run()
        process.waitUntilExit()
        #expect(process.terminationStatus == 0)
        return try Data(contentsOf: archive)
    }
}

@MainActor
private final class NoopDeleteSnapshotSink: ExtensionSnapshotSink {
    nonisolated func applyExtensionSnapshot(_: NotificationSocketServer.ExtensionSnapshot) {}
}
