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

    @Test("enabled localization providers refresh immediately")
    func enabledLocalizationsRefreshImmediately() throws {
        let root = try makeRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let name = "language-\(UUID().uuidString)"
        defer { ExtensionEnabledStore.clear(extensionID: name) }
        try makeLocalizationExtension(name: name, in: root)
        var refreshCount = 0
        let store = makeStore(root: root) { _ in refreshCount += 1 }

        store.loadManifestsIfNeeded()

        #expect(refreshCount == 1)
        #expect(store.localizations().isEmpty)

        store.setEnabled(true, for: name)

        #expect(refreshCount == 2)
        #expect(store.localizations().map(\.localization.id) == ["de"])

        store.setEnabled(false, for: name)

        #expect(refreshCount == 3)
        #expect(store.localizations().isEmpty)
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

    private func makeLocalizationExtension(name: String, in root: URL) throws {
        let directory = root.appendingPathComponent(name)
        let catalogDirectory = directory.appendingPathComponent("German.bundle/de.lproj")
        try FileManager.default.createDirectory(at: catalogDirectory, withIntermediateDirectories: true)
        let manifest = """
        {
            "name": "\(name)",
            "version": "1.0.0",
            "localizations": [
                {
                    "id": "de",
                    "language": "de",
                    "title": "Deutsch",
                    "bundle": "German.bundle"
                }
            ]
        }
        """
        try ExtensionManifestFixture.write(flatManifest: manifest, to: directory)
        try Data(resourceBundleInfo.utf8).write(to: directory.appendingPathComponent("German.bundle/Info.plist"))
        try Data(#""Settings" = "Einstellungen";"#.utf8)
            .write(to: catalogDirectory.appendingPathComponent("Localizable.strings"))
    }

    private func makeStore(
        root: URL,
        localizationsDidChange: @escaping @MainActor (ExtensionStore) -> Void = { _ in }
    ) -> ExtensionStore {
        ExtensionStore.makeForTesting(
            rootDirectory: root,
            snapshotSink: NoopManifestSnapshotSink(),
            resolveHostURL: { nil },
            localizationsDidChange: localizationsDidChange
        )
    }

    private var resourceBundleInfo: String {
        """
        <?xml version="1.0" encoding="UTF-8"?>
        <plist version="1.0">
        <dict>
            <key>CFBundleIdentifier</key>
            <string>app.muxy.localization.store-test</string>
            <key>CFBundleDevelopmentRegion</key>
            <string>de</string>
        </dict>
        </plist>
        """
    }
}

@MainActor
private final class NoopManifestSnapshotSink: ExtensionSnapshotSink {
    nonisolated func applyExtensionSnapshot(_: NotificationSocketServer.ExtensionSnapshot) {}
}
