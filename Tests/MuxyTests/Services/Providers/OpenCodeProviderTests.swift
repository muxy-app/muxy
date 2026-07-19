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

        let destinationURL = homeDirectory.appendingPathComponent(".opencode/plugins/muxy-notify.js")
        #expect(try Data(contentsOf: destinationURL) == Data("first".utf8))
        #expect(try permissions(of: destinationURL) == FilePermissions.privateFile)

        try Data("second".utf8).write(to: sourceURL)
        try provider.install(hookScriptPath: sourceURL.path)

        #expect(try Data(contentsOf: destinationURL) == Data("second".utf8))
    }

    private func permissions(of url: URL) throws -> Int {
        let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
        return try #require(attributes[.posixPermissions] as? NSNumber).intValue
    }
}
