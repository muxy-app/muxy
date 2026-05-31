import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionStore install")
@MainActor
struct ExtensionStoreInstallTests {
    @Test("unzips, validates, and registers the installed extension")
    func installsExtension() async throws {
        let root = makeRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = makeStore(root: root)

        let zip = try makeExtensionZip(name: "demo-ext", version: "1.0.0")

        try await store.install(expectedName: "demo-ext", zip: zip)

        #expect(store.statuses.contains { $0.id == "demo-ext" })
        let manifest = root.appendingPathComponent("demo-ext/manifest.json")
        #expect(FileManager.default.fileExists(atPath: manifest.path))
    }

    @Test("reinstall overwrites the existing directory")
    func reinstallOverwrites() async throws {
        let root = makeRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = makeStore(root: root)

        try await store.install(expectedName: "demo-ext", zip: makeExtensionZip(name: "demo-ext", version: "1.0.0"))
        try await store.install(expectedName: "demo-ext", zip: makeExtensionZip(name: "demo-ext", version: "2.0.0"))

        let status = try #require(store.statuses.first { $0.id == "demo-ext" })
        #expect(status.muxyExtension.manifest.version == "2.0.0")
    }

    @Test("rejects a package whose manifest name does not match")
    func rejectsNameMismatch() async throws {
        let root = makeRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = makeStore(root: root)

        let zip = try makeExtensionZip(name: "other-name", version: "1.0.0")

        await #expect(throws: MarketplaceError.self) {
            try await store.install(expectedName: "demo-ext", zip: zip)
        }
        #expect(!FileManager.default.fileExists(atPath: root.appendingPathComponent("demo-ext").path))
    }

    private func makeRoot() -> URL {
        FileManager.default.temporaryDirectory.appendingPathComponent("install-root-\(UUID().uuidString)")
    }

    private func makeStore(root: URL) -> ExtensionStore {
        ExtensionStore.makeForTesting(
            rootDirectory: root,
            snapshotSink: NoopSnapshotSink(),
            resolveHostURL: { URL(fileURLWithPath: "/usr/bin/true") }
        )
    }

    private func makeExtensionZip(name: String, version: String) throws -> Data {
        let fileManager = FileManager.default
        let workspace = fileManager.temporaryDirectory.appendingPathComponent("zip-src-\(UUID().uuidString)")
        let source = workspace.appendingPathComponent(name)
        try fileManager.createDirectory(at: source, withIntermediateDirectories: true)
        defer { try? fileManager.removeItem(at: workspace) }

        let manifest = """
        {
            "name": "\(name)",
            "version": "\(version)",
            "background": "background.js"
        }
        """
        try Data(manifest.utf8).write(to: source.appendingPathComponent("manifest.json"))
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
private final class NoopSnapshotSink: ExtensionSnapshotSink {
    nonisolated func applyExtensionSnapshot(_: NotificationSocketServer.ExtensionSnapshot) {}
}
