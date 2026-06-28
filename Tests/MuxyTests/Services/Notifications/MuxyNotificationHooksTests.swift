import Darwin
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

    @Test("shell hooks terminate socket notification payloads with newline")
    func shellHooksTerminateNotificationPayloadsWithNewline() throws {
        for scriptName in [
            "muxy-claude-hook.sh",
            "muxy-codex-hook.sh",
            "muxy-cursor-hook.sh",
            "muxy-droid-hook.sh",
            "muxy-grok-hook.sh",
        ] {
            let contents = try String(
                contentsOf: Self.repositoryRoot
                    .appendingPathComponent("Muxy/Resources/scripts/\(scriptName)"),
                encoding: .utf8
            )
            #expect(contents.contains("printf '%s|%s|%s|%s\\n'"))
        }
    }

    @Test("OpenCode plugin terminates socket notification payloads with newline")
    func openCodePluginTerminatesNotificationPayloadsWithNewline() throws {
        let contents = try String(
            contentsOf: Self.repositoryRoot
                .appendingPathComponent("Muxy/Resources/scripts/opencode-muxy-plugin.js"),
            encoding: .utf8
        )
        #expect(contents.contains("send(socketPath, `opencode|${paneID}|OpenCode|${sanitize(body)}`)"))
        #expect(contents.contains("conn.write(`${payload}\\n`"))
    }

    @Test("Pi extension terminates socket notification payloads with newline")
    func piExtensionTerminatesNotificationPayloadsWithNewline() throws {
        let contents = try String(
            contentsOf: Self.repositoryRoot
                .appendingPathComponent("Muxy/Resources/scripts/muxy-pi-extension.ts"),
            encoding: .utf8
        )
        #expect(contents.contains("send(`pi|${paneID}|Pi|${body}`)"))
        #expect(contents.contains("conn.write(`${payload}\\n`"))
    }

    @Test("shell hooks send newline terminated payloads to socket")
    func shellHooksSendNewlineTerminatedPayloadsToSocket() throws {
        for sample in Self.shellHookSamples {
            let payloads = try Self.runShellHook(sample)
            #expect(!payloads.isEmpty)
            for payload in payloads {
                #expect(payload.hasSuffix("\n"))
                #expect(payload.contains("|\(Self.paneID)|"))
            }
        }
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

    private static var repositoryRoot: URL {
        RepositoryRoot.find()
    }

    private static let paneID = UUID().uuidString

    private static let shellHookSamples = [
        ShellHookSample(scriptName: "muxy-claude-hook.sh", event: "stop", input: "{}"),
        ShellHookSample(scriptName: "muxy-codex-hook.sh", event: "stop", input: "{}"),
        ShellHookSample(scriptName: "muxy-cursor-hook.sh", event: "Stop", input: "{}"),
        ShellHookSample(scriptName: "muxy-droid-hook.sh", event: "stop", input: "{}"),
        ShellHookSample(scriptName: "muxy-grok-hook.sh", event: "stop", input: "{}"),
    ]

    private struct ShellHookSample {
        let scriptName: String
        let event: String
        let input: String
    }

    private static func runShellHook(_ sample: ShellHookSample) throws -> [String] {
        let socketPath = URL(fileURLWithPath: "/tmp")
            .appendingPathComponent("muxy-hook-\(UUID().uuidString).sock")
            .path
        let listener = try bindListener(at: socketPath)
        defer {
            close(listener)
            unlink(socketPath)
        }

        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/bash")
        process.arguments = [
            repositoryRoot
                .appendingPathComponent("Muxy/Resources/scripts/\(sample.scriptName)")
                .path,
            sample.event,
        ]
        var environment = ProcessInfo.processInfo.environment
        environment["MUXY_SOCKET_PATH"] = socketPath
        environment["MUXY_PANE_ID"] = paneID
        process.environment = environment
        let stdin = Pipe()
        process.standardInput = stdin

        try process.run()
        defer {
            if process.isRunning {
                process.terminate()
                process.waitUntilExit()
            }
        }
        stdin.fileHandleForWriting.write(Data(sample.input.utf8))
        try stdin.fileHandleForWriting.close()

        let payloads = drainConnections(listener, while: process)
        try waitForProcess(process)
        #expect(process.terminationStatus == 0)
        return payloads
    }

    private static func drainConnections(_ listener: Int32, while process: Process) -> [String] {
        var payloads: [String] = []
        let deadline = Date().addingTimeInterval(10)
        while Date() < deadline {
            var event = pollfd(fd: listener, events: Int16(POLLIN), revents: 0)
            let ready = poll(&event, 1, 500)
            if ready > 0 {
                let accepted = accept(listener, nil, nil)
                guard accepted >= 0 else { continue }
                let data = (try? readPayload(from: accepted)) ?? Data()
                close(accepted)
                if !data.isEmpty {
                    payloads.append(String(decoding: data, as: UTF8.self))
                }
                continue
            }
            if !process.isRunning { break }
        }
        return payloads
    }

    private static func bindListener(at path: String) throws -> Int32 {
        let descriptor = socket(AF_UNIX, SOCK_STREAM, 0)
        guard descriptor >= 0 else { throw POSIXError(.EMFILE) }
        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let capacity = MemoryLayout.size(ofValue: addr.sun_path)
        guard path.utf8.count < capacity else {
            close(descriptor)
            throw POSIXError(.ENAMETOOLONG)
        }
        withUnsafeMutablePointer(to: &addr.sun_path) { ptr in
            let bound = ptr.withMemoryRebound(to: CChar.self, capacity: capacity) { $0 }
            _ = path.withCString { strncpy(bound, $0, capacity - 1) }
        }

        let bindResult = withUnsafePointer(to: &addr) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                bind(descriptor, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard bindResult == 0, listen(descriptor, 5) == 0 else {
            close(descriptor)
            throw POSIXError(.EADDRINUSE)
        }
        return descriptor
    }

    private static func readPayload(from descriptor: Int32) throws -> Data {
        var data = Data()
        var buffer = [UInt8](repeating: 0, count: 4096)
        while !data.contains(10) {
            var event = pollfd(fd: descriptor, events: Int16(POLLIN), revents: 0)
            let ready = poll(&event, 1, 3_000)
            guard ready > 0 else { throw POSIXError(.ETIMEDOUT) }
            let count = read(descriptor, &buffer, buffer.count)
            guard count > 0 else { break }
            data.append(buffer, count: count)
        }
        return data
    }

    private static func waitForProcess(_ process: Process) throws {
        let deadline = Date().addingTimeInterval(3)
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
}
