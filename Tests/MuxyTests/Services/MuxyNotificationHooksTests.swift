import Foundation
import Testing

@testable import Muxy

@Suite("MuxyNotificationHooks")
struct MuxyNotificationHooksTests {
    @Test("findBundledScript finds file at bundle root")
    func findsFileAtBundleRoot() throws {
        let tmp = try temporaryBundle()
        defer { try? FileManager.default.removeItem(at: tmp) }

        let rootFile = tmp.appendingPathComponent("hook.sh")
        try Data("root".utf8).write(to: rootFile)

        let bundle = try #require(Bundle(url: tmp))
        let found = MuxyNotificationHooks.findBundledScript("hook", extension: "sh", bundle: bundle)

        #expect(found == rootFile.path)
    }

    @Test("findBundledScript falls back to scripts/ subdirectory")
    func findsFileInScriptsSubdirectory() throws {
        let tmp = try temporaryBundle()
        defer { try? FileManager.default.removeItem(at: tmp) }

        let scriptsDir = tmp.appendingPathComponent("scripts")
        try FileManager.default.createDirectory(at: scriptsDir, withIntermediateDirectories: true)
        let scriptFile = scriptsDir.appendingPathComponent("muxy-test-hook.sh")
        try Data("test".utf8).write(to: scriptFile)

        let bundle = try #require(Bundle(url: tmp))
        let found = MuxyNotificationHooks.findBundledScript("muxy-test-hook", extension: "sh", bundle: bundle)

        #expect(found == scriptFile.path)
    }

    @Test("findBundledScript returns nil when file does not exist")
    func returnsNilWhenNotFound() throws {
        let tmp = try temporaryBundle()
        defer { try? FileManager.default.removeItem(at: tmp) }

        let bundle = try #require(Bundle(url: tmp))
        let found = MuxyNotificationHooks.findBundledScript("nonexistent", extension: "ts", bundle: bundle)

        #expect(found == nil)
    }

    @Test("findBundledScript prefers root file over scripts/ subdirectory")
    func prefersRootOverScripts() throws {
        let tmp = try temporaryBundle()
        defer { try? FileManager.default.removeItem(at: tmp) }

        let rootFile = tmp.appendingPathComponent("dupe.sh")
        try Data("root".utf8).write(to: rootFile)

        let scriptsDir = tmp.appendingPathComponent("scripts")
        try FileManager.default.createDirectory(at: scriptsDir, withIntermediateDirectories: true)
        let scriptFile = scriptsDir.appendingPathComponent("dupe.sh")
        try Data("scripts".utf8).write(to: scriptFile)

        let bundle = try #require(Bundle(url: tmp))
        let found = MuxyNotificationHooks.findBundledScript("dupe", extension: "sh", bundle: bundle)

        #expect(found == rootFile.path)
    }

    private func temporaryBundle() throws -> URL {
        let tmp = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-test-bundle-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tmp, withIntermediateDirectories: true)
        let infoPlist = tmp.appendingPathComponent("Info.plist")
        let plist: [String: Any] = [
            "CFBundleIdentifier": "app.muxy.test",
            "CFBundleName": "TestBundle",
            "CFBundleVersion": "1",
            "CFBundlePackageType": "BNDL",
        ]
        let data = try PropertyListSerialization.data(fromPropertyList: plist, format: .xml, options: 0)
        try data.write(to: infoPlist)
        return tmp
    }
}
