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

    @Test("install creates wrapper and extension in Muxy bin without mutating Pi settings")
    func installCreatesWrapperAndExtension() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        try fixture.provider().install(hookScriptPath: "")

        let wrapperData = try Data(contentsOf: fixture.wrapperURL)
        let extensionData = try Data(contentsOf: fixture.extensionURL)
        #expect(wrapperData == fixture.wrapperSourceData)
        #expect(extensionData == fixture.extensionSourceData)
        #expect(FileManager.default.isExecutableFile(atPath: fixture.wrapperURL.path))

        let settings = try fixture.readSettings()
        #expect(settings["extensions"] as? [String] == [])
        #expect(!FileManager.default.fileExists(atPath: fixture.legacyExtensionURL.path))
    }

    @Test("install is idempotent when wrapper and extension are already current")
    func installIsIdempotent() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let provider = fixture.provider()

        try provider.install(hookScriptPath: "")
        let firstWrapperMod = try fixture.wrapperURL.resourceValues(forKeys: [.contentModificationDateKey])
        try provider.install(hookScriptPath: "")

        let secondWrapperMod = try fixture.wrapperURL.resourceValues(forKeys: [.contentModificationDateKey])
        #expect(firstWrapperMod.contentModificationDate == secondWrapperMod.contentModificationDate)
    }

    @Test("uninstall removes wrapper and extension from Muxy bin")
    func uninstallRemovesWrapperAndExtension() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let provider = fixture.provider()

        try provider.install(hookScriptPath: "")
        try provider.uninstall()

        #expect(!FileManager.default.fileExists(atPath: fixture.wrapperURL.path))
        #expect(!FileManager.default.fileExists(atPath: fixture.extensionURL.path))
    }

    @Test("uninstall does nothing when wrapper does not exist")
    func uninstallNoWrapper() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        try fixture.provider().uninstall()
    }

    @Test("install removes legacy global extension and settings registration")
    func installRemovesLegacyGlobalInstall() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        try FileManager.default.createDirectory(
            at: fixture.legacyExtensionURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try fixture.extensionSourceData.write(to: fixture.legacyExtensionURL)
        var settings = try fixture.readSettings()
        settings["extensions"] = [fixture.legacyExtensionURL.path]
        try fixture.writeSettings(settings)

        try fixture.provider().install(hookScriptPath: "")

        #expect(!FileManager.default.fileExists(atPath: fixture.legacyExtensionURL.path))
        let updatedSettings = try fixture.readSettings()
        let extensions = updatedSettings["extensions"] as? [String] ?? []
        #expect(!extensions.contains(fixture.legacyExtensionURL.path))
    }

    @Test("uninstall removes legacy global extension and settings registration")
    func uninstallRemovesLegacyGlobalInstall() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        try FileManager.default.createDirectory(
            at: fixture.legacyExtensionURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try fixture.extensionSourceData.write(to: fixture.legacyExtensionURL)
        try fixture.writeSettings(["extensions": [fixture.legacyExtensionURL.path]])

        try fixture.provider().uninstall()

        #expect(!FileManager.default.fileExists(atPath: fixture.legacyExtensionURL.path))
        let settings = try fixture.readSettings()
        #expect(settings["extensions"] == nil)
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

    @Test("isToolInstalled checks PATH entries but ignores Muxy wrapper")
    func isToolInstalledFromPathIgnoresMuxyWrapper() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        try fixture.provider().install(hookScriptPath: "")

        let realPiURL = fixture.rootURL.appendingPathComponent("npm/bin/pi")
        try FileManager.default.createDirectory(at: realPiURL.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data().write(to: realPiURL)
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.executable],
            ofItemAtPath: realPiURL.path
        )

        let path = "\(fixture.binDirectory.path):\(realPiURL.deletingLastPathComponent().path)"
        #expect(fixture.provider(pathEnvironment: path).isToolInstalled())
    }

    @Test("isToolInstalled evaluates PATH at call time")
    func isToolInstalledUsesCurrentPathEnvironment() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let pathEnvironment = PathEnvironment()
        let provider = fixture.provider(pathEnvironment: { pathEnvironment.value })

        let binURL = fixture.rootURL.appendingPathComponent("late-bin")
        let executableURL = binURL.appendingPathComponent("pi")
        try FileManager.default.createDirectory(at: binURL, withIntermediateDirectories: true)
        try Data().write(to: executableURL)
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.executable],
            ofItemAtPath: executableURL.path
        )
        pathEnvironment.value = binURL.path

        #expect(provider.isToolInstalled())
    }

    @Test("uninstall gracefully returns when legacy settings are missing")
    func unregisterGracefullyWhenSettingsMissing() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        try FileManager.default.removeItem(at: fixture.settingsURL)
        #expect(!FileManager.default.fileExists(atPath: fixture.settingsURL.path))

        try fixture.provider().uninstall()
    }

    @Test("install throws when extension resource is missing")
    func installThrowsWhenExtensionResourceMissing() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let provider = PiProvider(
            homeDirectory: fixture.homeURL.path,
            appSupportDirectory: fixture.appSupportURL,
            resourceURL: { _, ext in
                ext == "sh" ? fixture.wrapperSourceURL : nil
            }
        )

        #expect(throws: PiProviderError.bundleResourceNotFound) {
            try provider.install(hookScriptPath: "")
        }
    }

    @Test("install throws when wrapper resource is missing")
    func installThrowsWhenWrapperResourceMissing() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        let provider = PiProvider(
            homeDirectory: fixture.homeURL.path,
            appSupportDirectory: fixture.appSupportURL,
            resourceURL: { _, ext in
                ext == "ts" ? fixture.extensionSourceURL : nil
            }
        )

        #expect(throws: PiProviderError.bundleWrapperNotFound) {
            try provider.install(hookScriptPath: "")
        }
    }

    private final class PathEnvironment: @unchecked Sendable {
        private let lock = NSLock()
        private var storage = ""

        var value: String {
            get { lock.withLock { storage } }
            set { lock.withLock { storage = newValue } }
        }
    }

    private struct Fixture {
        let rootURL: URL
        let homeURL: URL
        let appSupportURL: URL
        let extensionSourceURL: URL
        let wrapperSourceURL: URL
        let settingsURL: URL
        let legacyExtensionURL: URL

        var binDirectory: URL { MuxyAgentBin.directory(appSupportDirectory: appSupportURL) }
        var wrapperURL: URL { MuxyAgentBin.wrapperURL(appSupportDirectory: appSupportURL) }
        var extensionURL: URL { MuxyAgentBin.extensionURL(appSupportDirectory: appSupportURL) }
        var extensionSourceData: Data { try! Data(contentsOf: extensionSourceURL) }
        var wrapperSourceData: Data { try! Data(contentsOf: wrapperSourceURL) }

        init() throws {
            rootURL = FileManager.default.temporaryDirectory
                .appendingPathComponent("PiProviderTests-\(UUID().uuidString)", isDirectory: true)
            homeURL = rootURL.appendingPathComponent("home", isDirectory: true)
            appSupportURL = rootURL.appendingPathComponent("Muxy", isDirectory: true)
            extensionSourceURL = rootURL.appendingPathComponent("muxy-pi-extension.ts")
            wrapperSourceURL = rootURL.appendingPathComponent("muxy-pi-wrapper.sh")
            settingsURL = homeURL.appendingPathComponent(".pi/agent/settings.json")
            legacyExtensionURL = homeURL.appendingPathComponent(".pi/agent/extensions/muxy-notify.ts")

            try FileManager.default.createDirectory(at: homeURL, withIntermediateDirectories: true)
            try Data("extension source".utf8).write(to: extensionSourceURL)
            try Data("#!/usr/bin/env bash\nexec pi \"$@\"\n".utf8).write(to: wrapperSourceURL)
            try FileManager.default.createDirectory(
                at: settingsURL.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            try writeSettings(["extensions": []])
        }

        func provider(pathEnvironment: String = "") -> PiProvider {
            provider(pathEnvironment: { pathEnvironment })
        }

        func provider(pathEnvironment: @escaping @Sendable () -> String) -> PiProvider {
            PiProvider(
                homeDirectory: homeURL.path,
                pathEnvironment: pathEnvironment,
                appSupportDirectory: appSupportURL,
                resourceURL: { name, ext in
                    switch (name, ext) {
                    case ("muxy-pi-extension", "ts"): extensionSourceURL
                    case ("muxy-pi-wrapper", "sh"): wrapperSourceURL
                    default: nil
                    }
                }
            )
        }

        func readSettings() throws -> [String: Any] {
            let data = try Data(contentsOf: settingsURL)
            return try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        }

        func writeSettings(_ settings: [String: Any]) throws {
            let data = try JSONSerialization.data(withJSONObject: settings, options: [.prettyPrinted, .sortedKeys])
            try data.write(to: settingsURL)
        }

        func cleanUp() {
            try? FileManager.default.removeItem(at: rootURL)
        }
    }
}
