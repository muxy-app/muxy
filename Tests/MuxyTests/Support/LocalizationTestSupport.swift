import Foundation

@testable import Muxy

@MainActor
enum LocalizationTestSupport {
    static func makeService(translations: String) throws -> (service: LocalizationService, root: URL) {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("localization-search-\(UUID().uuidString)")
        let bundleURL = root.appendingPathComponent("German.bundle")
        let catalogDirectory = bundleURL.appendingPathComponent("de.lproj")
        try FileManager.default.createDirectory(at: catalogDirectory, withIntermediateDirectories: true)
        try Data(bundleInfo.utf8).write(to: bundleURL.appendingPathComponent("Info.plist"))
        try Data(translations.utf8)
            .write(to: catalogDirectory.appendingPathComponent("Localizable.strings"))

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
        let binding = ExtensionStore.LocalizationBinding(
            muxyExtension: MuxyExtension(
                id: "german",
                directory: root,
                manifest: manifest
            ),
            localization: localization
        )
        let suiteName = "LocalizationTestSupport-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            throw CocoaError(.fileWriteUnknown)
        }
        defaults.removePersistentDomain(forName: suiteName)
        let service = LocalizationService(defaults: defaults)
        service.refresh(storedValue: binding.id, bindings: [binding])
        return (service, root)
    }

    private static let bundleInfo = """
    <?xml version="1.0" encoding="UTF-8"?>
    <plist version="1.0">
    <dict>
        <key>CFBundleIdentifier</key>
        <string>app.muxy.localization.search.test</string>
        <key>CFBundleDevelopmentRegion</key>
        <string>de</string>
    </dict>
    </plist>
    """
}
