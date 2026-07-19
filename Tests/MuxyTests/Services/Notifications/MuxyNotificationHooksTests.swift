import Foundation
import Testing

@testable import Muxy

@Suite("MuxyNotificationHooks")
struct MuxyNotificationHooksTests {
    @Test("findBundledScript finds file at bundle root")
    func findsFileAtBundleRoot() throws {
        let bundleDirectory = try temporaryBundle()
        defer { try? FileManager.default.removeItem(at: bundleDirectory) }
        let fileURL = bundleDirectory.appendingPathComponent("hook.sh")
        try Data("root".utf8).write(to: fileURL)

        let bundle = try #require(Bundle(url: bundleDirectory))

        #expect(MuxyNotificationHooks.findBundledScript("hook", extension: "sh", bundle: bundle) == fileURL.path)
    }

    @Test("findBundledScript falls back to scripts subdirectory")
    func findsFileInScriptsSubdirectory() throws {
        let bundleDirectory = try temporaryBundle()
        defer { try? FileManager.default.removeItem(at: bundleDirectory) }
        let scriptsDirectory = bundleDirectory.appendingPathComponent("scripts", isDirectory: true)
        try FileManager.default.createDirectory(at: scriptsDirectory, withIntermediateDirectories: true)
        let fileURL = scriptsDirectory.appendingPathComponent("muxy-test-hook.sh")
        try Data("test".utf8).write(to: fileURL)

        let bundle = try #require(Bundle(url: bundleDirectory))

        #expect(
            MuxyNotificationHooks.findBundledScript("muxy-test-hook", extension: "sh", bundle: bundle)
                == fileURL.path
        )
    }

    @Test("findBundledScript returns nil when file does not exist")
    func returnsNilWhenNotFound() throws {
        let bundleDirectory = try temporaryBundle()
        defer { try? FileManager.default.removeItem(at: bundleDirectory) }
        let bundle = try #require(Bundle(url: bundleDirectory))

        #expect(MuxyNotificationHooks.findBundledScript("nonexistent", extension: "ts", bundle: bundle) == nil)
    }

    @Test("staging refreshes every hook resource at stable private paths")
    func stagesAndRefreshesHookResources() throws {
        let fixture = try StagingFixture()
        defer { fixture.cleanUp() }

        #expect(MuxyNotificationHooks.stageAll(
            bundle: fixture.bundle,
            hookBinaryURL: fixture.binaryURL,
            destinationDirectory: fixture.destinationDirectory,
            searchDevelopmentDirectory: false
        ))

        #expect(try permissions(of: fixture.destinationDirectory) == FilePermissions.privateDirectory)
        #expect(try permissions(of: fixture.stagedBinaryURL) == FilePermissions.privateExecutable)
        #expect(FileManager.default.isExecutableFile(atPath: fixture.stagedBinaryURL.path))
        for scriptName in Self.shellScriptNames {
            let scriptURL = fixture.destinationDirectory.appendingPathComponent(scriptName)
            #expect(try permissions(of: scriptURL) == FilePermissions.privateExecutable)
        }
        for sourceName in ["opencode-muxy-plugin.js", "muxy-pi-extension.ts"] {
            let sourceURL = fixture.destinationDirectory.appendingPathComponent(sourceName)
            #expect(try permissions(of: sourceURL) == FilePermissions.privateFile)
        }

        try Data("updated binary".utf8).write(to: fixture.binaryURL)
        let updatedScriptURL = fixture.scriptsDirectory.appendingPathComponent("muxy-codex-hook.sh")
        try Data("updated script".utf8).write(to: updatedScriptURL)

        #expect(MuxyNotificationHooks.stageAll(
            bundle: fixture.bundle,
            hookBinaryURL: fixture.binaryURL,
            destinationDirectory: fixture.destinationDirectory,
            searchDevelopmentDirectory: false
        ))
        #expect(try Data(contentsOf: fixture.stagedBinaryURL) == Data("updated binary".utf8))
        #expect(
            try Data(contentsOf: fixture.destinationDirectory.appendingPathComponent("muxy-codex-hook.sh"))
                == Data("updated script".utf8)
        )

        try FileManager.default.removeItem(at: fixture.bundleDirectory)
        #expect(FileManager.default.fileExists(atPath: fixture.stagedBinaryURL.path))
        #expect(FileManager.default.fileExists(atPath: fixture.destinationDirectory
            .appendingPathComponent("muxy-codex-hook.sh").path))
    }

    @Test("staging fails without the compiled bridge")
    func stagingRequiresCompiledBridge() throws {
        let fixture = try StagingFixture()
        defer { fixture.cleanUp() }

        #expect(!MuxyNotificationHooks.stageAll(
            bundle: fixture.bundle,
            hookBinaryURL: nil,
            destinationDirectory: fixture.destinationDirectory,
            searchDevelopmentDirectory: false
        ))
        #expect(!FileManager.default.fileExists(atPath: fixture.stagedBinaryURL.path))
    }

    @Test("shell shims invoke the colocated compiled bridge")
    func shellShimsInvokeCompiledBridge() throws {
        for scriptName in Self.shellScriptNames {
            let scriptURL = Self.repositoryRoot.appendingPathComponent("Muxy/Resources/scripts/\(scriptName)")
            let contents = try String(contentsOf: scriptURL, encoding: .utf8)
            #expect(contents.contains("$(dirname \"$0\")/muxy-hook"))
            #expect(contents.contains("agent-event --provider"))
            #expect(!contents.contains("muxy-agent-hook.sh"))
        }
        #expect(!FileManager.default.fileExists(atPath: Self.repositoryRoot
            .appendingPathComponent("Muxy/Resources/scripts/muxy-agent-hook.sh").path))
    }

    @Test("OpenCode invokes the bridge and logs when the binary is missing")
    func openCodeUsesBridgeOnly() throws {
        let contents = try String(
            contentsOf: Self.repositoryRoot.appendingPathComponent("Muxy/Resources/scripts/opencode-muxy-plugin.js"),
            encoding: .utf8
        )

        #expect(contents.contains("process.env.MUXY_HOOK_BIN"))
        #expect(contents.contains("node:child_process"))
        #expect(contents.contains("agent-event"))
        #expect(contents.contains("sendQueue = sendQueue.then(transmit, transmit)"))
        #expect(contents.contains("muxy-hook binary is not staged"))
        #expect(!contents.contains("agent_status|"))
        #expect(!contents.contains("agent_event|"))
        #expect(!contents.contains("createConnection"))
        #expect(!contents.contains("MUXY_AGENT_EVENT_PROTOCOL"))
    }

    @Test("Pi invokes the bridge and logs when the binary is missing")
    func piUsesBridgeOnly() throws {
        let contents = try String(
            contentsOf: Self.repositoryRoot.appendingPathComponent("Muxy/Resources/scripts/muxy-pi-extension.ts"),
            encoding: .utf8
        )

        #expect(contents.contains("process.env.MUXY_HOOK_BIN"))
        #expect(contents.contains("node:child_process"))
        #expect(contents.contains("agent-event"))
        #expect(contents.contains("muxy-hook binary is not staged"))
        #expect(!contents.contains("agent_status|"))
        #expect(!contents.contains("agent_event|"))
        #expect(!contents.contains("createConnection"))
        #expect(!contents.contains("MUXY_AGENT_EVENT_PROTOCOL"))
    }

    private func temporaryBundle() throws -> URL {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-test-bundle-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let plist: [String: Any] = [
            "CFBundleIdentifier": "app.muxy.test",
            "CFBundleName": "TestBundle",
            "CFBundleVersion": "1",
            "CFBundlePackageType": "BNDL",
        ]
        let data = try PropertyListSerialization.data(fromPropertyList: plist, format: .xml, options: 0)
        try data.write(to: directory.appendingPathComponent("Info.plist"))
        return directory
    }

    private func permissions(of url: URL) throws -> Int {
        let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
        return try #require(attributes[.posixPermissions] as? NSNumber).intValue
    }

    private static var repositoryRoot: URL {
        RepositoryRoot.find()
    }

    private static let shellScriptNames = [
        "muxy-claude-hook.sh",
        "muxy-codex-hook.sh",
        "muxy-cursor-hook.sh",
        "muxy-droid-hook.sh",
        "muxy-grok-hook.sh",
    ]
    private struct StagingFixture {
        let rootDirectory: URL
        let bundleDirectory: URL
        let scriptsDirectory: URL
        let binaryURL: URL
        let destinationDirectory: URL
        let bundle: Bundle

        var stagedBinaryURL: URL {
            destinationDirectory.appendingPathComponent(MuxyNotificationHooks.hookBinaryName)
        }

        init() throws {
            rootDirectory = FileManager.default.temporaryDirectory
                .appendingPathComponent("MuxyNotificationHooksTests-\(UUID().uuidString)", isDirectory: true)
            bundleDirectory = rootDirectory.appendingPathComponent("Test.bundle", isDirectory: true)
            scriptsDirectory = bundleDirectory.appendingPathComponent("scripts", isDirectory: true)
            binaryURL = rootDirectory.appendingPathComponent("source-muxy-hook")
            destinationDirectory = rootDirectory.appendingPathComponent("Application Support/hooks", isDirectory: true)
            try FileManager.default.createDirectory(at: scriptsDirectory, withIntermediateDirectories: true)
            let plist: [String: Any] = [
                "CFBundleIdentifier": "app.muxy.hook-tests",
                "CFBundleName": "HookTests",
                "CFBundleVersion": "1",
                "CFBundlePackageType": "BNDL",
            ]
            let plistData = try PropertyListSerialization.data(fromPropertyList: plist, format: .xml, options: 0)
            try plistData.write(to: bundleDirectory.appendingPathComponent("Info.plist"))
            for scriptName in MuxyNotificationHooksTests.shellScriptNames + [
                "opencode-muxy-plugin.js",
                "muxy-pi-extension.ts",
            ] {
                try Data("source \(scriptName)".utf8).write(to: scriptsDirectory.appendingPathComponent(scriptName))
            }
            try Data("binary".utf8).write(to: binaryURL)
            bundle = try #require(Bundle(url: bundleDirectory))
        }

        func cleanUp() {
            try? FileManager.default.removeItem(at: rootDirectory)
        }
    }
}
