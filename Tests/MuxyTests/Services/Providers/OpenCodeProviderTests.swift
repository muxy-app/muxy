import Foundation
import Testing

@testable import Muxy

@Suite("OpenCodeProvider")
struct OpenCodeProviderTests {
    @Test("provider requests its staged plugin resource")
    func stagedPluginIdentity() {
        let provider = OpenCodeProvider()

        #expect(provider.hookScriptName == "opencode-muxy-plugin")
        #expect(provider.hookScriptExtension == "js")
    }

    @Test("managed and obsolete plugin paths stay separate")
    func configPathsSeparateObsoleteLocation() {
        let homeDirectory = "/Users/example"
        let provider = OpenCodeProvider(homeDirectory: homeDirectory, pathEnvironment: "")

        #expect(provider.configPaths == [homeDirectory + "/.config/opencode/plugins/muxy-notify.js"])
        #expect(provider.obsoleteConfigPaths == [homeDirectory + "/.opencode/plugins/muxy-notify.js"])
    }

    @Test("discovery recognizes the managed global plugin")
    func discoveryRecognizesManagedPlugin() {
        let homeDirectory = "/Users/example"
        let provider = OpenCodeProvider(homeDirectory: homeDirectory, pathEnvironment: "")
        let pluginURL = URL(
            fileURLWithPath: homeDirectory + "/.config/opencode/plugins/muxy-notify.js"
        ).absoluteString

        let details = provider.discoveryDetails(from: """
        opencode version: 1.18.5
        plugins:
        - \(pluginURL)
        """)

        #expect(details == ProviderDiscoveryDetails(version: "1.18.5", state: .ready))
    }

    @Test("discovery reports legacy and missing plugins")
    func discoveryReportsPluginProblems() {
        let homeDirectory = "/Users/example"
        let provider = OpenCodeProvider(homeDirectory: homeDirectory, pathEnvironment: "")
        let legacyURL = URL(
            fileURLWithPath: homeDirectory + "/.opencode/plugins/muxy-notify.js"
        ).absoluteString

        #expect(provider.discoveryDetails(from: """
        opencode version: 1.18.5
        plugins:
        - \(legacyURL)
        """) == ProviderDiscoveryDetails(
            version: "1.18.5",
            state: .warning("Legacy Muxy plugin discovered")
        ))
        #expect(provider.discoveryDetails(from: """
        opencode version: 1.18.5
        diagnostics:
        - \(URL(fileURLWithPath: homeDirectory + "/.config/opencode/plugins/muxy-notify.js").absoluteString)
        plugins:
        none
        """) == ProviderDiscoveryDetails(
            version: "1.18.5",
            state: .warning("Muxy plugin not discovered by OpenCode")
        ))
    }

    @Test("install copies and refreshes the supplied staged plugin")
    func installUsesSuppliedStagedPlugin() throws {
        let rootDirectory = FileManager.default.temporaryDirectory
            .appendingPathComponent("OpenCodeProviderTests-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: rootDirectory) }
        let homeDirectory = rootDirectory.appendingPathComponent("home", isDirectory: true)
        let sourceURL = rootDirectory.appendingPathComponent("opencode-muxy-plugin.js")
        try FileManager.default.createDirectory(at: homeDirectory, withIntermediateDirectories: true)
        try Data("first".utf8).write(to: sourceURL)
        let provider = OpenCodeProvider(homeDirectory: homeDirectory.path, pathEnvironment: "")

        try provider.install(hookScriptPath: sourceURL.path)

        let destinationURL = homeDirectory.appendingPathComponent(".config/opencode/plugins/muxy-notify.js")
        #expect(try Data(contentsOf: destinationURL) == Data("first".utf8))
        #expect(try permissions(of: destinationURL) == FilePermissions.privateFile)

        try Data("second".utf8).write(to: sourceURL)
        try provider.install(hookScriptPath: sourceURL.path)

        #expect(try Data(contentsOf: destinationURL) == Data("second".utf8))
    }

    @Test("install migrates the obsolete global plugin location")
    func installMigratesLegacyPlugin() throws {
        let rootDirectory = FileManager.default.temporaryDirectory
            .appendingPathComponent("OpenCodeProviderTests-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: rootDirectory) }
        let homeDirectory = rootDirectory.appendingPathComponent("home", isDirectory: true)
        let sourceURL = rootDirectory.appendingPathComponent("opencode-muxy-plugin.js")
        let legacyURL = homeDirectory.appendingPathComponent(".opencode/plugins/muxy-notify.js")
        try FileManager.default.createDirectory(
            at: legacyURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try Data("legacy".utf8).write(to: legacyURL)
        try Data("current".utf8).write(to: sourceURL)
        let provider = OpenCodeProvider(homeDirectory: homeDirectory.path, pathEnvironment: "")

        #expect(provider.hasManagedState())
        #expect(provider.verify(hookScriptPath: sourceURL.path) == .needsRepair)

        try provider.install(hookScriptPath: sourceURL.path)

        let destinationURL = homeDirectory.appendingPathComponent(".config/opencode/plugins/muxy-notify.js")
        #expect(try Data(contentsOf: destinationURL) == Data("current".utf8))
        #expect(!FileManager.default.fileExists(atPath: legacyURL.path))
        #expect(provider.verify(hookScriptPath: sourceURL.path) == .satisfied)
    }

    @Test("uninstall removes current and obsolete plugin copies")
    func uninstallRemovesManagedPlugins() throws {
        let rootDirectory = FileManager.default.temporaryDirectory
            .appendingPathComponent("OpenCodeProviderTests-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: rootDirectory) }
        let homeDirectory = rootDirectory.appendingPathComponent("home", isDirectory: true)
        let currentURL = homeDirectory.appendingPathComponent(".config/opencode/plugins/muxy-notify.js")
        let legacyURL = homeDirectory.appendingPathComponent(".opencode/plugins/muxy-notify.js")
        for url in [currentURL, legacyURL] {
            try FileManager.default.createDirectory(
                at: url.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            try Data("managed".utf8).write(to: url)
        }
        let provider = OpenCodeProvider(homeDirectory: homeDirectory.path, pathEnvironment: "")

        try provider.uninstall()

        #expect(!FileManager.default.fileExists(atPath: currentURL.path))
        #expect(!FileManager.default.fileExists(atPath: legacyURL.path))
        #expect(!provider.hasManagedState())
    }

    private func permissions(of url: URL) throws -> Int {
        let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
        return try #require(attributes[.posixPermissions] as? NSNumber).intValue
    }
}
