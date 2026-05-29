import Foundation
import Testing

@testable import Muxy

@Suite("PiProvider")
struct PiProviderTests {
    private let provider = PiProvider()

    @Test("id returns pi")
    func id() {
        #expect(provider.id == "pi")
    }

    @Test("displayName returns Pi")
    func displayName() {
        #expect(provider.displayName == "Pi")
    }

    @Test("socketTypeKey returns pi")
    func socketTypeKey() {
        #expect(provider.socketTypeKey == "pi")
    }

    @Test("iconName returns pi")
    func iconName() {
        #expect(provider.iconName == "pi")
    }

    @Test("executableNames contains pi")
    func executableNames() {
        #expect(provider.executableNames == ["pi"])
    }

    @Test("hookScriptName returns muxy-pi-extension")
    func hookScriptName() {
        #expect(provider.hookScriptName == "muxy-pi-extension")
    }

    @Test("settingsKey is derived from id")
    func settingsKey() {
        #expect(provider.settingsKey == "muxy.notifications.provider.pi.enabled")
    }

    @Test("isEnabled stores and retrieves value via UserDefaults")
    func isEnabledStorage() {
        let key = provider.settingsKey
        let defaults = UserDefaults.standard

        defaults.removeObject(forKey: key)
        #expect(defaults.bool(forKey: key, fallback: true) == true)

        provider.isEnabled = false
        #expect(provider.isEnabled == false)

        provider.isEnabled = true
        #expect(provider.isEnabled == true)

        defaults.removeObject(forKey: key)
    }

    @Test("install creates extension file and registers settings")
    func installCreatesFileAndRegistersSettings() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let provider = fixture.provider()

        try provider.install(hookScriptPath: "")

        let destinationURL = fixture.homeURL
            .appendingPathComponent(".pi/agent/extensions/muxy-notify.ts")
        let installedData = try Data(contentsOf: destinationURL)
        let sourceData = try Data(contentsOf: fixture.sourceURL)
        #expect(installedData == sourceData)

        let settings = try fixture.readSettings()
        let extensions = try #require(settings["extensions"] as? [String])
        #expect(extensions == [destinationURL.path])
        #expect(FileManager.default.fileExists(atPath: fixture.settingsURL.path + ".muxy-backup"))
    }

    @Test("install is idempotent when extension is already current")
    func installIsIdempotent() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let provider = fixture.provider()

        try provider.install(hookScriptPath: "")
        try provider.install(hookScriptPath: "")

        let settings = try fixture.readSettings()
        let extensions = try #require(settings["extensions"] as? [String])
        #expect(extensions.count == 1)
    }

    @Test("uninstall removes extension file and unregisters settings")
    func uninstallRemovesFileAndUnregistersSettings() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let provider = fixture.provider()

        try provider.install(hookScriptPath: "")
        try provider.uninstall()

        let destinationPath = fixture.homeURL
            .appendingPathComponent(".pi/agent/extensions/muxy-notify.ts")
            .path
        #expect(!FileManager.default.fileExists(atPath: destinationPath))

        let settings = try fixture.readSettings()
        #expect(settings["extensions"] == nil)
    }

    @Test("uninstall does nothing when file does not exist")
    func uninstallNoFile() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        try fixture.provider().uninstall()
    }

    @Test("isToolInstalled checks common paths")
    func isToolInstalledFromCommonPath() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let executableURL = fixture.homeURL.appendingPathComponent(".local/bin/pi")
        try FileManager.default.createDirectory(
            at: executableURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try Data().write(to: executableURL)
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.executable],
            ofItemAtPath: executableURL.path
        )

        #expect(fixture.provider().isToolInstalled())
    }

    @Test("isToolInstalled checks PATH entries")
    func isToolInstalledFromPath() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let binURL = fixture.rootURL.appendingPathComponent("npm/bin")
        let executableURL = binURL.appendingPathComponent("pi")
        try FileManager.default.createDirectory(at: binURL, withIntermediateDirectories: true)
        try Data().write(to: executableURL)
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.executable],
            ofItemAtPath: executableURL.path
        )

        #expect(fixture.provider(pathEnvironment: binURL.path).isToolInstalled())
    }

    @Test("registerExtensionInSettings creates settings.json when it does not exist")
    func registerCreatesSettingsWhenMissing() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        try FileManager.default.removeItem(at: fixture.settingsURL)
        #expect(!FileManager.default.fileExists(atPath: fixture.settingsURL.path))

        try fixture.provider().install(hookScriptPath: "")

        #expect(FileManager.default.fileExists(atPath: fixture.settingsURL.path))
        let settings = try fixture.readSettings()
        let extensions = try #require(settings["extensions"] as? [String])
        #expect(extensions.count == 1)
    }

    @Test("registerExtensionInSettings throws invalidSettingsFile when settings.json is not valid JSON")
    func registerThrowsWhenSettingsInvalid() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        try Data("not json".utf8).write(to: fixture.settingsURL)

        #expect(throws: PiProviderError.invalidSettingsFile(fixture.settingsURL.path)) {
            try fixture.provider().install(hookScriptPath: "")
        }
    }

    @Test("unregisterExtensionFromSettings gracefully returns when settings.json is missing")
    func unregisterGracefullyWhenSettingsMissing() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let provider = fixture.provider()
        try provider.install(hookScriptPath: "")

        try FileManager.default.removeItem(at: fixture.settingsURL)
        #expect(!FileManager.default.fileExists(atPath: fixture.settingsURL.path))

        try provider.uninstall()
    }

    @Test("install throws when resource is missing")
    func installThrowsWhenResourceMissing() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let provider = PiProvider(
            homeDirectory: fixture.homeURL.path,
            pathEnvironment: "",
            resourceURL: { _, _ in nil }
        )

        #expect(throws: PiProviderError.bundleResourceNotFound) {
            try provider.install(hookScriptPath: "")
        }
    }

    private struct Fixture {
        let rootURL: URL
        let homeURL: URL
        let sourceURL: URL
        let settingsURL: URL

        init() throws {
            rootURL = FileManager.default.temporaryDirectory
                .appendingPathComponent("PiProviderTests-\(UUID().uuidString)", isDirectory: true)
            homeURL = rootURL.appendingPathComponent("home", isDirectory: true)
            sourceURL = rootURL.appendingPathComponent("muxy-pi-extension.ts")
            settingsURL = homeURL.appendingPathComponent(".pi/agent/settings.json")

            try FileManager.default.createDirectory(
                at: settingsURL.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            try Data("extension source".utf8).write(to: sourceURL)
            let settingsData = try JSONSerialization.data(
                withJSONObject: ["extensions": []],
                options: [.prettyPrinted, .sortedKeys]
            )
            try settingsData.write(to: settingsURL)
        }

        func provider(pathEnvironment: String = "") -> PiProvider {
            PiProvider(
                homeDirectory: homeURL.path,
                pathEnvironment: pathEnvironment,
                resourceURL: { _, _ in sourceURL }
            )
        }

        func readSettings() throws -> [String: Any] {
            let data = try Data(contentsOf: settingsURL)
            return try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        }

        func cleanUp() {
            try? FileManager.default.removeItem(at: rootURL)
        }
    }
}
