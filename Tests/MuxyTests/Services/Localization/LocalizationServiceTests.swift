import Foundation
import Testing

@testable import Muxy

@Suite("LocalizationService")
@MainActor
struct LocalizationServiceTests {
    @Test("uses selected extension bundle and falls back to English")
    func resolvesSelectedBundleAndEnglishFallback() throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("localization-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: root) }
        let bundleURL = root.appendingPathComponent("German.bundle")
        let catalogDirectory = bundleURL.appendingPathComponent("de.lproj")
        try FileManager.default.createDirectory(at: catalogDirectory, withIntermediateDirectories: true)
        try Data(resourceBundleInfo.utf8).write(to: bundleURL.appendingPathComponent("Info.plist"))
        try Data(
            """
            "Settings" = "Einstellungen";
            "Open %@" = "%@ öffnen";
            """.utf8
        )
            .write(to: catalogDirectory.appendingPathComponent("Localizable.strings"))

        let binding = makeBinding(root: root)
        let suiteName = "LocalizationServiceTests-\(UUID().uuidString)"
        let defaults = try #require(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let service = LocalizationService(defaults: defaults)
        defaults.set(binding.id, forKey: LocalizationSelection.storageKey)

        service.refresh(storedValue: binding.id, bindings: [binding])

        #expect(service.string("Settings") == "Einstellungen")
        #expect(service.string("Missing translation") == "Missing translation")
        #expect(service.string("Open \("Project")") == "Project öffnen")

        service.refresh(storedValue: binding.id, bindings: [])

        #expect(service.activeSelection == LocalizationSelection.builtinValue)
        #expect(service.string("Settings") == "Settings")
        #expect(defaults.string(forKey: LocalizationSelection.storageKey) == binding.id)
    }

    private func makeBinding(root: URL) -> ExtensionStore.LocalizationBinding {
        let localization = ExtensionLocalization(
            id: "de",
            language: "de",
            title: "Deutsch",
            bundle: "German.bundle"
        )
        let manifest = ExtensionManifest(
            name: "german",
            version: "1.0.0",
            localizations: [localization]
        )
        return ExtensionStore.LocalizationBinding(
            muxyExtension: MuxyExtension(
                id: "german",
                directory: root,
                manifest: manifest
            ),
            localization: localization
        )
    }

    private var resourceBundleInfo: String {
        """
        <?xml version="1.0" encoding="UTF-8"?>
        <plist version="1.0">
        <dict>
            <key>CFBundleIdentifier</key>
            <string>app.muxy.localization.test</string>
            <key>CFBundleDevelopmentRegion</key>
            <string>de</string>
        </dict>
        </plist>
        """
    }
}
