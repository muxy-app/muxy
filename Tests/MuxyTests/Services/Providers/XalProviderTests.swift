import Foundation
import Testing

@testable import Muxy

@Suite("XalProvider")
struct XalProviderTests {
    private let provider = XalProvider()

    @Test("identity matches the Xal CLI")
    func identity() {
        #expect(provider.id == "xal")
        #expect(provider.displayName == "Xal")
        #expect(provider.socketTypeKey == "xal")
        #expect(provider.iconName == "xal")
        #expect(provider.executableNames == ["xal"])
        #expect(provider.hookScriptName == "muxy-xal-plugin")
        #expect(provider.hookScriptExtension == "ts")
    }

    @Test("headless launch runs a single text turn")
    func headlessLaunchConfiguration() {
        let invocation = provider.agentLaunchConfiguration.invocation(prompt: "Summarize", model: "gpt-5.6-terra")
        #expect(invocation?.executable == "xal")
        #expect(invocation?.arguments == ["run", "--format", "text", "--model", "gpt-5.6-terra", "Summarize"])
    }

    @Test("settingsKey is derived from id")
    func settingsKey() {
        #expect(provider.settingsKey == "muxy.notifications.provider.xal.enabled")
    }

    @Test("install stages the plugin and registers its directory")
    func installStagesPluginAndRegistersDirectory() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        let provider = fixture.provider()

        try provider.install(hookScriptPath: fixture.sourceURL.path)

        #expect(try Data(contentsOf: fixture.pluginURL) == Data(contentsOf: fixture.sourceURL))
        #expect(try fixture.permissions(of: fixture.pluginURL) == FilePermissions.privateFile)
        let plugins = try #require(fixture.readConfiguration()["plugins"] as? [String])
        #expect(plugins == [fixture.pluginDirectoryURL.path])
        #expect(provider.verify(hookScriptPath: fixture.sourceURL.path) == .satisfied)
    }

    @Test("install preserves unrelated configuration and foreign plugins")
    func installPreservesForeignConfiguration() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        try fixture.writeConfiguration([
            "model": "gpt-5.6-terra",
            "plugins": ["/opt/other-plugin"],
        ])

        try fixture.provider().install(hookScriptPath: fixture.sourceURL.path)

        let configuration = fixture.readConfiguration()
        #expect(configuration["model"] as? String == "gpt-5.6-terra")
        #expect(configuration["plugins"] as? [String] == ["/opt/other-plugin", fixture.pluginDirectoryURL.path])
    }

    @Test("install is idempotent")
    func installIsIdempotent() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        let provider = fixture.provider()

        try provider.install(hookScriptPath: fixture.sourceURL.path)
        try provider.install(hookScriptPath: fixture.sourceURL.path)

        #expect(fixture.readConfiguration()["plugins"] as? [String] == [fixture.pluginDirectoryURL.path])
    }

    @Test("install refreshes a stale plugin from the staged source")
    func installRefreshesStalePlugin() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        let provider = fixture.provider()

        try provider.install(hookScriptPath: fixture.sourceURL.path)
        try Data("updated plugin source".utf8).write(to: fixture.sourceURL)
        #expect(provider.verify(hookScriptPath: fixture.sourceURL.path) == .needsRepair)

        try provider.install(hookScriptPath: fixture.sourceURL.path)

        #expect(try Data(contentsOf: fixture.pluginURL) == Data("updated plugin source".utf8))
    }

    @Test("verify needs repair when the registration was removed")
    func verifyNeedsRepairWhenRegistrationRemoved() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        let provider = fixture.provider()

        try provider.install(hookScriptPath: fixture.sourceURL.path)
        try fixture.writeConfiguration(["plugins": []])

        #expect(provider.verify(hookScriptPath: fixture.sourceURL.path) == .needsRepair)
        #expect(provider.hasManagedState())
    }

    @Test("uninstall removes the plugin directory and its registration")
    func uninstallRemovesPluginAndRegistration() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        let provider = fixture.provider()
        try fixture.writeConfiguration(["plugins": ["/opt/other-plugin"]])

        try provider.install(hookScriptPath: fixture.sourceURL.path)
        try provider.uninstall()

        #expect(!FileManager.default.fileExists(atPath: fixture.pluginDirectoryURL.path))
        #expect(fixture.readConfiguration()["plugins"] as? [String] == ["/opt/other-plugin"])
        #expect(!provider.hasManagedState())
    }

    @Test("uninstall drops the plugins key when Muxy was the only entry")
    func uninstallDropsEmptyPluginsKey() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        let provider = fixture.provider()

        try provider.install(hookScriptPath: fixture.sourceURL.path)
        try provider.uninstall()

        #expect(fixture.readConfiguration()["plugins"] == nil)
    }

    @Test("uninstall clears a stale registration when the plugin file is absent")
    func uninstallClearsStaleRegistration() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        try fixture.writeConfiguration(["plugins": [fixture.pluginDirectoryURL.path]])
        let provider = fixture.provider()

        #expect(provider.hasManagedState())
        try provider.uninstall()

        #expect(fixture.readConfiguration()["plugins"] == nil)
    }

    @Test("uninstall does nothing without managed state")
    func uninstallWithoutManagedState() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        try fixture.provider().uninstall()

        #expect(!FileManager.default.fileExists(atPath: fixture.configurationURL.path))
    }

    @Test("uninstall preserves the plugin when configuration writing fails")
    func uninstallPreservesPluginOnConfigurationWriteFailure() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        try fixture.provider().install(hookScriptPath: fixture.sourceURL.path)
        let originalConfiguration = try Data(contentsOf: fixture.configurationURL)
        let originalPlugin = try Data(contentsOf: fixture.pluginURL)
        let failingWriter = FailingConfigurationWriter()
        let provider = fixture.provider(configurationWriter: failingWriter.write)

        #expect(throws: ConfigurationWriteError.failed) {
            try provider.uninstall()
        }
        #expect(try Data(contentsOf: fixture.configurationURL) == originalConfiguration)
        #expect(try Data(contentsOf: fixture.pluginURL) == originalPlugin)
        #expect(provider.hasManagedState())
    }

    @Test("uninstall restores configuration when plugin deletion fails")
    func uninstallRestoresConfigurationOnPluginDeletionFailure() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        try fixture.provider().install(hookScriptPath: fixture.sourceURL.path)
        let originalConfiguration = try Data(contentsOf: fixture.configurationURL)
        let originalPlugin = try Data(contentsOf: fixture.pluginURL)
        let provider = fixture.provider(pluginDirectoryRemover: { _ in
            throw PluginRemovalError.failed
        })

        #expect(throws: PluginRemovalError.failed) {
            try provider.uninstall()
        }
        #expect(try Data(contentsOf: fixture.configurationURL) == originalConfiguration)
        #expect(try Data(contentsOf: fixture.pluginURL) == originalPlugin)
        #expect(provider.hasManagedState())
    }

    @Test("install throws when the staged resource is missing")
    func installThrowsWhenResourceMissing() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }

        #expect(throws: XalProviderError.hookResourceNotFound) {
            try fixture.provider().install(
                hookScriptPath: fixture.rootURL.appendingPathComponent("missing.ts").path
            )
        }
    }

    @Test("install throws instead of overwriting a malformed configuration")
    func installThrowsOnMalformedConfiguration() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        try fixture.writeRawConfiguration("not json")

        #expect(throws: XalProviderError.malformedConfiguration(fixture.configurationURL.path)) {
            try fixture.provider().install(hookScriptPath: fixture.sourceURL.path)
        }
        #expect(try Data(contentsOf: fixture.configurationURL) == Data("not json".utf8))
        #expect(!FileManager.default.fileExists(atPath: fixture.pluginDirectoryURL.path))
    }

    @Test("install rejects a non-array plugins value without changing configuration")
    func installRejectsInvalidPluginsConfiguration() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        try fixture.writeConfiguration(["plugins": "/opt/other-plugin"])
        let originalConfiguration = try Data(contentsOf: fixture.configurationURL)

        #expect(throws: XalProviderError.invalidPluginsConfiguration(fixture.configurationURL.path)) {
            try fixture.provider().install(hookScriptPath: fixture.sourceURL.path)
        }
        #expect(try Data(contentsOf: fixture.configurationURL) == originalConfiguration)
        #expect(!FileManager.default.fileExists(atPath: fixture.pluginDirectoryURL.path))
    }

    @Test("install removes a new plugin directory when configuration writing fails")
    func installRollsBackNewPluginOnConfigurationWriteFailure() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        let provider = fixture.provider(configurationWriter: { _, _ in
            throw ConfigurationWriteError.failed
        })

        #expect(throws: ConfigurationWriteError.failed) {
            try provider.install(hookScriptPath: fixture.sourceURL.path)
        }
        #expect(!FileManager.default.fileExists(atPath: fixture.pluginDirectoryURL.path))
    }

    @Test("install restores an existing plugin when configuration writing fails")
    func installRestoresPluginOnConfigurationWriteFailure() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        try FileManager.default.createDirectory(at: fixture.pluginDirectoryURL, withIntermediateDirectories: true)
        let originalData = Data("existing plugin".utf8)
        try originalData.write(to: fixture.pluginURL)
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.executable],
            ofItemAtPath: fixture.pluginURL.path
        )
        let provider = fixture.provider(configurationWriter: { _, _ in
            throw ConfigurationWriteError.failed
        })

        #expect(throws: ConfigurationWriteError.failed) {
            try provider.install(hookScriptPath: fixture.sourceURL.path)
        }
        #expect(try Data(contentsOf: fixture.pluginURL) == originalData)
        #expect(try fixture.permissions(of: fixture.pluginURL) == FilePermissions.executable)
    }

    @Test("isToolInstalled finds the CLI in the default install location")
    func isToolInstalledFromHomeBin() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        try fixture.writeExecutable(at: fixture.homeURL.appendingPathComponent(".local/bin/xal"))

        #expect(fixture.provider().isToolInstalled())
    }

    @Test("isToolInstalled finds the CLI on PATH")
    func isToolInstalledFromPath() throws {
        let fixture = try Fixture()
        defer { fixture.cleanUp() }
        let binURL = fixture.rootURL.appendingPathComponent("bin")
        try fixture.writeExecutable(at: binURL.appendingPathComponent("xal"))

        #expect(fixture.provider(pathEnvironment: binURL.path).isToolInstalled())
    }

    private enum ConfigurationWriteError: Error, Equatable {
        case failed
    }

    private enum PluginRemovalError: Error, Equatable {
        case failed
    }

    private final class FailingConfigurationWriter {
        private var shouldFail = true

        func write(_ configuration: [String: Any], _ path: String) throws {
            try HookConfigWriter.write(configuration, to: path)
            guard shouldFail else { return }
            shouldFail = false
            throw ConfigurationWriteError.failed
        }
    }

    private struct Fixture {
        let rootURL: URL
        let homeURL: URL
        let sourceURL: URL
        let configurationURL: URL
        let pluginDirectoryURL: URL

        var pluginURL: URL { pluginDirectoryURL.appendingPathComponent("plugin.ts") }

        init() throws {
            rootURL = FileManager.default.temporaryDirectory
                .appendingPathComponent("XalProviderTests-\(UUID().uuidString)", isDirectory: true)
            homeURL = rootURL.appendingPathComponent("home", isDirectory: true)
            sourceURL = rootURL.appendingPathComponent("muxy-xal-plugin.ts")
            configurationURL = homeURL.appendingPathComponent(".xal/config.json")
            pluginDirectoryURL = homeURL.appendingPathComponent(".xal/plugins/muxy-notify", isDirectory: true)

            try FileManager.default.createDirectory(at: homeURL, withIntermediateDirectories: true)
            try Data("plugin source".utf8).write(to: sourceURL)
        }

        func provider(
            pathEnvironment: String = "",
            configurationWriter: @escaping ([String: Any], String) throws -> Void = {
                try HookConfigWriter.write($0, to: $1)
            },
            pluginDirectoryRemover: @escaping (String) throws -> Void = {
                try FileManager.default.removeItem(atPath: $0)
            }
        ) -> XalProvider {
            XalProvider(
                homeDirectory: homeURL.path,
                pathEnvironment: pathEnvironment,
                configurationWriter: configurationWriter,
                pluginDirectoryRemover: pluginDirectoryRemover
            )
        }

        func writeConfiguration(_ configuration: [String: Any]) throws {
            try FileManager.default.createDirectory(
                at: configurationURL.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            let data = try JSONSerialization.data(
                withJSONObject: configuration,
                options: [.prettyPrinted, .sortedKeys]
            )
            try data.write(to: configurationURL)
        }

        func writeRawConfiguration(_ contents: String) throws {
            try FileManager.default.createDirectory(
                at: configurationURL.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            try Data(contents.utf8).write(to: configurationURL)
        }

        func readConfiguration() -> [String: Any] {
            guard let data = try? Data(contentsOf: configurationURL),
                  let configuration = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
            else { return [:] }
            return configuration
        }

        func writeExecutable(at url: URL) throws {
            try FileManager.default.createDirectory(
                at: url.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            try Data().write(to: url)
            try FileManager.default.setAttributes(
                [.posixPermissions: FilePermissions.executable],
                ofItemAtPath: url.path
            )
        }

        func permissions(of url: URL) throws -> Int {
            let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
            return try #require(attributes[.posixPermissions] as? NSNumber).intValue
        }

        func cleanUp() {
            try? FileManager.default.removeItem(at: rootURL)
        }
    }
}
