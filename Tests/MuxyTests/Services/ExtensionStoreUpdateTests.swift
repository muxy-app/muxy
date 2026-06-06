import CryptoKit
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

    @Test("update installs the remote version and clears the flag")
    func updateInstallsRemoteVersion() async throws {
        let root = makeRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = makeStore(root: root, versions: ["demo-ext": "2.0.0"])
        StubMarketplaceURLProtocol.packages["demo-ext"] = try makeExtensionZip(name: "demo-ext", version: "2.0.0")

        try await store.install(expectedName: "demo-ext", zip: makeExtensionZip(name: "demo-ext", version: "1.0.0"))
        await store.checkForUpdates()
        #expect(store.hasUpdates)

        try await store.update(extensionID: "demo-ext")

        #expect(!store.hasUpdates)
        #expect(store.statuses.first(where: { $0.id == "demo-ext" })?.muxyExtension.manifest.version == "2.0.0")
    }

    @Test("updateAll reports successes and failures separately")
    func updateAllAggregatesResults() async throws {
        let root = makeRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = makeStore(root: root, versions: ["ok-ext": "2.0.0", "fail-ext": "2.0.0"])
        StubMarketplaceURLProtocol.packages["ok-ext"] = try makeExtensionZip(name: "ok-ext", version: "2.0.0")

        try await store.install(expectedName: "ok-ext", zip: makeExtensionZip(name: "ok-ext", version: "1.0.0"))
        try await store.install(expectedName: "fail-ext", zip: makeExtensionZip(name: "fail-ext", version: "1.0.0"))
        await store.checkForUpdates()
        #expect(store.updateCount == 2)

        let result = await store.updateAll()

        #expect(result.succeeded == ["ok-ext"])
        #expect(result.failed.map(\.id) == ["fail-ext"])
        #expect(store.availableUpdateVersion(for: "ok-ext") == nil)
        #expect(store.availableUpdateVersion(for: "fail-ext") == "2.0.0")
    }

    @Test("does not flag dev extensions for updates")
    func ignoresDevExtensions() async throws {
        let root = makeRoot()
        let devParent = makeRoot()
        defer {
            try? FileManager.default.removeItem(at: root)
            try? FileManager.default.removeItem(at: devParent)
        }
        let devDir = try makeDevExtension(name: "demo-ext", version: "1.0.0", in: devParent)
        let store = makeStore(root: root, versions: ["demo-ext": "2.0.0"], devPaths: [devDir.path])

        store.startAll()
        await store.checkForUpdates()

        #expect(store.statuses.first(where: { $0.id == "demo-ext" })?.isDev == true)
        #expect(!store.hasUpdates)
    }

    private func makeRoot() -> URL {
        FileManager.default.temporaryDirectory.appendingPathComponent("update-root-\(UUID().uuidString)")
    }

    private func makeDevExtension(name: String, version: String, in parent: URL) throws -> URL {
        let directory = parent.appendingPathComponent("\(name)-dev")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let manifest = """
        {
            "name": "\(name)",
            "version": "\(version)"
        }
        """
        try ExtensionManifestFixture.write(flatManifest: manifest, to: directory)
        return directory
    }

    private func makeStore(root: URL, versions: [String: String], devPaths: [String] = []) -> ExtensionStore {
        StubMarketplaceURLProtocol.reset()
        StubMarketplaceURLProtocol.versions = versions
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubMarketplaceURLProtocol.self]
        let marketplace = ExtensionMarketplaceService(
            baseURL: URL(string: "https://muxy.test")!,
            session: URLSession(configuration: configuration)
        )
        return ExtensionStore.makeForTesting(
            rootDirectory: root,
            snapshotSink: NoopUpdateSnapshotSink(),
            resolveHostURL: { URL(fileURLWithPath: "/usr/bin/true") },
            marketplace: marketplace,
            devPathsProvider: { devPaths }
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
private final class NoopUpdateSnapshotSink: ExtensionSnapshotSink {
    nonisolated func applyExtensionSnapshot(_: NotificationSocketServer.ExtensionSnapshot) {}
}

private final class StubMarketplaceURLProtocol: URLProtocol {
    nonisolated(unsafe) static var versions: [String: String] = [:]
    nonisolated(unsafe) static var packages: [String: Data] = [:]

    static func reset() {
        versions = [:]
        packages = [:]
    }

    static func downloadURL(for name: String) -> String { "https://muxy.test/download/\(name)" }

    override class func canInit(with _: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        guard let url = request.url else {
            client?.urlProtocol(self, didFailWithError: URLError(.badServerResponse))
            return
        }
        let (status, data) = response(for: url)
        let httpResponse = HTTPURLResponse(url: url, statusCode: status, httpVersion: nil, headerFields: nil)!
        client?.urlProtocol(self, didReceive: httpResponse, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: data)
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}

    private func response(for url: URL) -> (Int, Data) {
        let path = url.path
        if path.hasSuffix("/api/extensions/versions") {
            let map = Self.versions.mapValues { Optional($0) }
            return (200, (try? JSONSerialization.data(withJSONObject: map)) ?? Data("{}".utf8))
        }
        if path.hasPrefix("/download/") {
            let name = url.lastPathComponent
            guard let package = Self.packages[name] else { return (404, Data()) }
            return (200, package)
        }
        if path.hasPrefix("/api/extensions/") {
            let name = url.lastPathComponent
            guard let package = Self.packages[name] else { return (404, Data("{}".utf8)) }
            return (200, Self.envelope(name: name, package: package))
        }
        return (404, Data())
    }

    private static func envelope(name: String, package: Data) -> Data {
        let sha = SHA256.hash(data: package).map { String(format: "%02x", $0) }.joined()
        let json = """
        {
            "data": {
                "name": "\(name)",
                "description": null,
                "permissions": [],
                "author": null,
                "homepage": null,
                "repository": null,
                "categories": [],
                "icon_url": null,
                "screenshot_paths": [],
                "downloads": 0,
                "current_version": "2.0.0",
                "sha256": "\(sha)",
                "size": \(package.count),
                "download_url": "\(downloadURL(for: name))"
            }
        }
        """
        return Data(json.utf8)
    }
}
