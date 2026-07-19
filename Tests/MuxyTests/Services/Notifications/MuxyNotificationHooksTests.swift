import Darwin
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

    @Test("OpenCode invokes the bridge and keeps direct v2 as missing-binary fallback")
    func openCodeUsesBridgeWithV2Fallback() throws {
        let contents = try String(
            contentsOf: Self.repositoryRoot.appendingPathComponent("Muxy/Resources/scripts/opencode-muxy-plugin.js"),
            encoding: .utf8
        )

        #expect(contents.contains("process.env.MUXY_HOOK_BIN"))
        #expect(contents.contains("node:child_process"))
        #expect(contents.contains("agent-event"))
        #expect(contents.contains("`agent_event|opencode|${paneID}|${phase}|"))
        #expect(contents.contains("sendQueue = sendQueue.then(transmit, transmit)"))
        #expect(!contents.contains("agent_status|"))
        #expect(!contents.contains("MUXY_AGENT_EVENT_PROTOCOL"))
    }

    @Test("Pi invokes the bridge and keeps direct v2 as missing-binary fallback")
    func piUsesBridgeWithV2Fallback() throws {
        let contents = try String(
            contentsOf: Self.repositoryRoot.appendingPathComponent("Muxy/Resources/scripts/muxy-pi-extension.ts"),
            encoding: .utf8
        )

        #expect(contents.contains("process.env.MUXY_HOOK_BIN"))
        #expect(contents.contains("node:child_process"))
        #expect(contents.contains("agent-event"))
        #expect(contents.contains("`agent_event|pi|${paneID}|${phase}|${title}|${body}`"))
        #expect(!contents.contains("agent_status|"))
        #expect(!contents.contains("MUXY_AGENT_EVENT_PROTOCOL"))
    }

    @Test("staged shims execute the SPM bridge and receive an ack")
    func stagedShimsExecuteBuiltBridge() throws {
        let binaryURL = Self.repositoryRoot.appendingPathComponent(".build/debug/muxy-hook")
        try #require(FileManager.default.isExecutableFile(atPath: binaryURL.path))
        let bundleDirectory = try temporaryBundle()
        defer { try? FileManager.default.removeItem(at: bundleDirectory) }
        let scriptsDirectory = bundleDirectory.appendingPathComponent("scripts", isDirectory: true)
        try FileManager.default.createDirectory(at: scriptsDirectory, withIntermediateDirectories: true)
        for scriptName in Self.shellScriptNames + [
            "opencode-muxy-plugin.js",
            "muxy-pi-extension.ts",
        ] {
            try FileManager.default.copyItem(
                at: Self.repositoryRoot.appendingPathComponent("Muxy/Resources/scripts/\(scriptName)"),
                to: scriptsDirectory.appendingPathComponent(scriptName)
            )
        }
        let bundle = try #require(Bundle(url: bundleDirectory))
        let destinationDirectory = FileManager.default.temporaryDirectory
            .appendingPathComponent("Muxy Hook E2E \(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: destinationDirectory) }
        try #require(MuxyNotificationHooks.stageAll(
            bundle: bundle,
            hookBinaryURL: binaryURL,
            destinationDirectory: destinationDirectory,
            searchDevelopmentDirectory: false
        ))

        for sample in Self.shellHookSamples {
            let message = try Self.runStagedShellHook(sample, in: destinationDirectory)
            #expect(message["v"] as? Int == 3)
            #expect(message["kind"] as? String == "agent_event")
            #expect(message["provider"] as? String == sample.provider)
            #expect(message["paneID"] as? String == Self.paneID)
            #expect(message["phase"] as? String == "finished")
        }
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

    private static let paneID = UUID().uuidString
    private static let shellScriptNames = [
        "muxy-claude-hook.sh",
        "muxy-codex-hook.sh",
        "muxy-cursor-hook.sh",
        "muxy-droid-hook.sh",
        "muxy-grok-hook.sh",
    ]
    private static let shellHookSamples = [
        ShellHookSample(scriptName: "muxy-claude-hook.sh", provider: "claude_hook", event: "stop"),
        ShellHookSample(scriptName: "muxy-codex-hook.sh", provider: "codex_hook", event: "stop"),
        ShellHookSample(scriptName: "muxy-cursor-hook.sh", provider: "cursor_hook", event: "Stop"),
        ShellHookSample(scriptName: "muxy-droid-hook.sh", provider: "droid_hook", event: "stop"),
        ShellHookSample(scriptName: "muxy-grok-hook.sh", provider: "grok_hook", event: "stop"),
    ]

    private static func runStagedShellHook(
        _ sample: ShellHookSample,
        in directory: URL
    ) throws -> [String: Any] {
        let socketPath = FileManager.default.temporaryDirectory
            .appendingPathComponent("mh-\(getpid())-\(Int.random(in: 0 ..< 1_000_000)).sock")
            .path
        let listener = try bindListener(at: socketPath)
        defer {
            close(listener)
            unlink(socketPath)
        }

        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/bash")
        process.arguments = [directory.appendingPathComponent(sample.scriptName).path, sample.event]
        var environment = ProcessInfo.processInfo.environment
        environment["MUXY_SOCKET_PATH"] = socketPath
        environment["MUXY_PANE_ID"] = paneID
        environment["MUXY_AGENT_EVENT_PROTOCOL"] = "3"
        process.environment = environment
        let standardInput = Pipe()
        process.standardInput = standardInput

        try process.run()
        standardInput.fileHandleForWriting.write(Data("{}".utf8))
        try standardInput.fileHandleForWriting.close()

        let accepted = try acceptConnection(listener)
        let data = try readPayload(from: accepted)
        try writeAck(to: accepted)
        close(accepted)
        try waitForProcess(process)
        #expect(process.terminationStatus == 0)

        return try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
    }

    private static func bindListener(at path: String) throws -> Int32 {
        let descriptor = socket(AF_UNIX, SOCK_STREAM, 0)
        guard descriptor >= 0 else { throw POSIXError(.EMFILE) }
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        let capacity = MemoryLayout.size(ofValue: address.sun_path)
        guard path.utf8.count < capacity else {
            close(descriptor)
            throw POSIXError(.ENAMETOOLONG)
        }
        withUnsafeMutablePointer(to: &address.sun_path) { pointer in
            let bound = pointer.withMemoryRebound(to: CChar.self, capacity: capacity) { $0 }
            _ = path.withCString { strncpy(bound, $0, capacity - 1) }
        }
        let result = withUnsafePointer(to: &address) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                bind(descriptor, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard result == 0 else {
            close(descriptor)
            throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
        }
        guard listen(descriptor, 1) == 0 else {
            close(descriptor)
            throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
        }
        return descriptor
    }

    private static func acceptConnection(_ listener: Int32) throws -> Int32 {
        var event = pollfd(fd: listener, events: Int16(POLLIN), revents: 0)
        guard poll(&event, 1, 5_000) > 0 else { throw POSIXError(.ETIMEDOUT) }
        let accepted = accept(listener, nil, nil)
        guard accepted >= 0 else { throw POSIXError(.ECONNABORTED) }
        return accepted
    }

    private static func readPayload(from descriptor: Int32) throws -> Data {
        var data = Data()
        var buffer = [UInt8](repeating: 0, count: 4096)
        while !data.contains(10) {
            var event = pollfd(fd: descriptor, events: Int16(POLLIN), revents: 0)
            guard poll(&event, 1, 3_000) > 0 else { throw POSIXError(.ETIMEDOUT) }
            let count = read(descriptor, &buffer, buffer.count)
            guard count > 0 else { break }
            data.append(buffer, count: count)
        }
        return data
    }

    private static func writeAck(to descriptor: Int32) throws {
        let data = Data(#"{"v":3,"kind":"ack","ok":true}"#.utf8) + Data([10])
        let written = data.withUnsafeBytes { pointer in
            Darwin.write(descriptor, pointer.baseAddress, pointer.count)
        }
        guard written == data.count else { throw POSIXError(.EIO) }
    }

    private static func waitForProcess(_ process: Process) throws {
        let deadline = Date().addingTimeInterval(5)
        while process.isRunning, Date() < deadline {
            Thread.sleep(forTimeInterval: 0.01)
        }
        guard !process.isRunning else {
            process.terminate()
            process.waitUntilExit()
            throw POSIXError(.ETIMEDOUT)
        }
        process.waitUntilExit()
    }

    private struct ShellHookSample {
        let scriptName: String
        let provider: String
        let event: String
    }

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
