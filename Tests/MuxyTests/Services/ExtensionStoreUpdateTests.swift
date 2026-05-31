import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionStore updates", .serialized)
@MainActor
struct ExtensionStoreUpdateTests {
    @Test("flags an extension whose remote version is newer")
    func detectsAvailableUpdate() async throws {
        let root = makeRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = makeStore(root: root, versions: ["demo-ext": "2.0.0"])

        try await store.install(expectedName: "demo-ext", zip: makeExtensionZip(name: "demo-ext", version: "1.0.0"))
        await store.checkForUpdates()

        #expect(store.hasUpdates)
        #expect(store.availableUpdateVersion(for: "demo-ext") == "2.0.0")
    }

    @Test("does not flag when remote version matches installed")
    func ignoresUpToDate() async throws {
        let root = makeRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = makeStore(root: root, versions: ["demo-ext": "1.0.0"])

        try await store.install(expectedName: "demo-ext", zip: makeExtensionZip(name: "demo-ext", version: "1.0.0"))
        await store.checkForUpdates()

        #expect(!store.hasUpdates)
    }

    @Test("clears a stale update entry after reinstalling a newer version")
    func prunesAfterReinstall() async throws {
        let root = makeRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = makeStore(root: root, versions: ["demo-ext": "2.0.0"])

        try await store.install(expectedName: "demo-ext", zip: makeExtensionZip(name: "demo-ext", version: "1.0.0"))
        await store.checkForUpdates()
        #expect(store.hasUpdates)

        try await store.install(expectedName: "demo-ext", zip: makeExtensionZip(name: "demo-ext", version: "2.0.0"))

        #expect(!store.hasUpdates)
    }

    private func makeRoot() -> URL {
        FileManager.default.temporaryDirectory.appendingPathComponent("update-root-\(UUID().uuidString)")
    }

    private func makeStore(root: URL, versions: [String: String]) -> ExtensionStore {
        StubVersionsURLProtocol.versions = versions
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubVersionsURLProtocol.self]
        let marketplace = ExtensionMarketplaceService(
            baseURL: URL(string: "https://muxy.test")!,
            session: URLSession(configuration: configuration)
        )
        return ExtensionStore.makeForTesting(
            rootDirectory: root,
            snapshotSink: NoopUpdateSnapshotSink(),
            resolveHostURL: { URL(fileURLWithPath: "/usr/bin/true") },
            marketplace: marketplace
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
private final class NoopUpdateSnapshotSink: ExtensionSnapshotSink {
    nonisolated func applyExtensionSnapshot(_: NotificationSocketServer.ExtensionSnapshot) {}
}

private final class StubVersionsURLProtocol: URLProtocol {
    nonisolated(unsafe) static var versions: [String: String] = [:]

    override class func canInit(with _: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        guard let url = request.url else {
            client?.urlProtocol(self, didFailWithError: URLError(.badServerResponse))
            return
        }
        let map = Self.versions.mapValues { Optional($0) }
        let data = (try? JSONSerialization.data(withJSONObject: map)) ?? Data("{}".utf8)
        let response = HTTPURLResponse(url: url, statusCode: 200, httpVersion: nil, headerFields: nil)!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: data)
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}
}
