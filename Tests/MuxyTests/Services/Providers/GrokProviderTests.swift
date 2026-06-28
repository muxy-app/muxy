import Darwin
import Foundation
import Testing

@testable import Muxy

@Suite("GrokProvider hooks")
struct GrokProviderTests {
    private func commands(script: String) -> [(settingsKey: String, command: String)] {
        GrokProvider.hookEvents.map {
            (settingsKey: $0.settingsKey, command: GrokProvider.hookCommand(hookScript: script, event: $0.event))
        }
    }

    private func nonMuxyEntry(command: String) -> [[String: Any]] {
        [["matcher": "", "hooks": [["type": "command", "command": command]]]]
    }

    @Test("provider identity matches expected wire and settings ids")
    func providerIdentity() {
        let provider = GrokProvider()
        #expect(provider.id == "grok")
        #expect(provider.displayName == "Grok")
        #expect(provider.socketTypeKey == "grok_hook")
        #expect(provider.iconName == "grok")
        #expect(provider.executableNames == ["grok"])
        #expect(provider.hookScriptName == "muxy-grok-hook")
        #expect(provider.hookScriptExtension == "sh")
    }

    @Test("hook command embeds the event argument and muxy marker")
    func hookCommandFormat() {
        let command = GrokProvider.hookCommand(hookScript: "/tmp/muxy-grok-hook.sh", event: "stop")
        #expect(command == "'/tmp/muxy-grok-hook.sh' stop # muxy-notification-hook")
    }

    @Test("installs the working, waiting and idle events into empty settings")
    func installsIntoEmpty() {
        let hooks = GrokProvider.hooks(installing: commands(script: "/tmp/hook.sh"), into: [:])
        for key in ["Stop", "Notification", "UserPromptSubmit", "PreToolUse"] {
            #expect((hooks?[key] as? [[String: Any]])?.count == 1)
        }
    }

    @Test("installing again is idempotent")
    func installIsIdempotent() {
        let cmds = commands(script: "/tmp/hook.sh")
        let installed = GrokProvider.hooks(installing: cmds, into: [:])!
        #expect(GrokProvider.hooks(installing: cmds, into: installed) == nil)
    }

    @Test("install preserves existing non-muxy hooks")
    func installPreservesForeignHooks() {
        let existing: [String: Any] = ["Stop": nonMuxyEntry(command: "echo hi")]
        let result = GrokProvider.hooks(installing: commands(script: "/tmp/hook.sh"), into: existing)!
        #expect((result["Stop"] as? [[String: Any]])?.count == 2)
    }

    @Test("reinstall with a new script path replaces the stale entry without duplicating")
    func reinstallReplacesStaleEntry() {
        let installed = GrokProvider.hooks(installing: commands(script: "/old/hook.sh"), into: [:])!
        let reinstalled = GrokProvider.hooks(installing: commands(script: "/new/hook.sh"), into: installed)!
        let preToolUse = reinstalled["PreToolUse"] as? [[String: Any]]
        #expect(preToolUse?.count == 1)
        let command = (preToolUse?.first?["hooks"] as? [[String: Any]])?.first?["command"] as? String
        #expect(command?.contains("/new/hook.sh") == true)
    }

    @Test("uninstall removes every muxy entry and drops emptied keys")
    func uninstallRemovesAll() {
        let installed = GrokProvider.hooks(installing: commands(script: "/tmp/hook.sh"), into: [:])!
        let cleaned = GrokProvider.hooks(uninstallingFrom: installed)
        #expect(cleaned.isEmpty)
    }

    @Test("uninstall keeps foreign hooks intact")
    func uninstallPreservesForeignHooks() {
        let existing: [String: Any] = ["Stop": nonMuxyEntry(command: "echo hi")]
        let installed = GrokProvider.hooks(installing: commands(script: "/tmp/hook.sh"), into: existing)!
        let cleaned = GrokProvider.hooks(uninstallingFrom: installed)
        let stop = cleaned["Stop"] as? [[String: Any]]
        #expect(stop?.count == 1)
        let command = (stop?.first?["hooks"] as? [[String: Any]])?.first?["command"] as? String
        #expect(command == "echo hi")
    }

    @Test("install writes muxy-notify.json under the injectable home hooks dir")
    func installWritesHooksFile() throws {
        try withTempHome { home in
            let script = home.appendingPathComponent("muxy-grok-hook.sh").path
            try "#!/bin/sh\n".write(toFile: script, atomically: true, encoding: .utf8)
            let provider = GrokProvider(homeDirectory: home.path, pathEnvironment: "")
            try provider.install(hookScriptPath: script)

            let hookURL = home.appendingPathComponent(".grok/hooks/muxy-notify.json")
            #expect(FileManager.default.fileExists(atPath: hookURL.path))

            let data = try Data(contentsOf: hookURL)
            let json = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
            let hooks = try #require(json["hooks"] as? [String: Any])
            for key in ["Stop", "Notification", "UserPromptSubmit", "PreToolUse"] {
                let entries = try #require(hooks[key] as? [[String: Any]])
                #expect(entries.count == 1)
                let command = (entries.first?["hooks"] as? [[String: Any]])?.first?["command"] as? String
                #expect(command?.contains(script) == true)
                #expect(command?.contains("muxy-notification-hook") == true)
            }
        }
    }

    @Test("install is a no-op when the file already matches")
    func installSkipsWhenAlreadyCurrent() throws {
        try withTempHome { home in
            let script = home.appendingPathComponent("muxy-grok-hook.sh").path
            try "#!/bin/sh\n".write(toFile: script, atomically: true, encoding: .utf8)
            let provider = GrokProvider(homeDirectory: home.path, pathEnvironment: "")
            try provider.install(hookScriptPath: script)
            let hookURL = home.appendingPathComponent(".grok/hooks/muxy-notify.json")
            let first = try Data(contentsOf: hookURL)
            try provider.install(hookScriptPath: script)
            let second = try Data(contentsOf: hookURL)
            #expect(first == second)
        }
    }

    @Test("uninstall strips muxy hooks but keeps the file and other root keys")
    func uninstallStripsHooksKeepsOtherKeys() throws {
        try withTempHome { home in
            let hooksDir = home.appendingPathComponent(".grok/hooks", isDirectory: true)
            try FileManager.default.createDirectory(at: hooksDir, withIntermediateDirectories: true)
            let hookURL = hooksDir.appendingPathComponent("muxy-notify.json")
            let preExisting: [String: Any] = [
                "version": 1,
                "hooks": [
                    "Stop": nonMuxyEntry(command: "echo foreign"),
                ],
            ]
            let data = try JSONSerialization.data(withJSONObject: preExisting, options: [.prettyPrinted, .sortedKeys])
            try data.write(to: hookURL)

            let script = home.appendingPathComponent("muxy-grok-hook.sh").path
            try "#!/bin/sh\n".write(toFile: script, atomically: true, encoding: .utf8)
            let provider = GrokProvider(homeDirectory: home.path, pathEnvironment: "")
            try provider.install(hookScriptPath: script)
            try provider.uninstall()

            #expect(FileManager.default.fileExists(atPath: hookURL.path))
            let remaining = try Data(contentsOf: hookURL)
            let json = try #require(JSONSerialization.jsonObject(with: remaining) as? [String: Any])
            #expect(json["version"] as? Int == 1)
            let hooks = try #require(json["hooks"] as? [String: Any])
            #expect(hooks["Notification"] == nil)
            let stop = try #require(hooks["Stop"] as? [[String: Any]])
            let command = (stop.first?["hooks"] as? [[String: Any]])?.first?["command"] as? String
            #expect(command == "echo foreign")
        }
    }

    @Test("uninstall with only muxy hooks removes the hooks key but keeps the file")
    func uninstallMuxyOnlyRemovesHooksKey() throws {
        try withTempHome { home in
            let script = home.appendingPathComponent("muxy-grok-hook.sh").path
            try "#!/bin/sh\n".write(toFile: script, atomically: true, encoding: .utf8)
            let provider = GrokProvider(homeDirectory: home.path, pathEnvironment: "")
            try provider.install(hookScriptPath: script)
            let hookURL = home.appendingPathComponent(".grok/hooks/muxy-notify.json")
            #expect(FileManager.default.fileExists(atPath: hookURL.path))
            try provider.uninstall()
            #expect(FileManager.default.fileExists(atPath: hookURL.path))
            let remaining = try Data(contentsOf: hookURL)
            let json = try #require(JSONSerialization.jsonObject(with: remaining) as? [String: Any])
            #expect(json["hooks"] == nil)
        }
    }

    @Test("install uses MuxyNotificationHooks.scriptPath for the shipped muxy-grok-hook resource")
    func installUsesShippedHookScriptPath() throws {
        try withTempHome { home in
            let scriptPath = try #require(
                MuxyNotificationHooks.scriptPath(named: "muxy-grok-hook", extension: "sh")
                    ?? Self.repositoryScriptPath()
            )
            #expect(scriptPath.hasSuffix("muxy-grok-hook.sh"))
            #expect(FileManager.default.fileExists(atPath: scriptPath))

            let provider = GrokProvider(homeDirectory: home.path, pathEnvironment: "")
            try provider.install(hookScriptPath: scriptPath)

            let hookURL = home.appendingPathComponent(".grok/hooks/muxy-notify.json")
            let data = try Data(contentsOf: hookURL)
            let json = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
            let hooks = try #require(json["hooks"] as? [String: Any])
            let stop = try #require(hooks["Stop"] as? [[String: Any]])
            let command = (stop.first?["hooks"] as? [[String: Any]])?.first?["command"] as? String
            #expect(command?.contains("muxy-grok-hook.sh") == true)
            #expect(command?.contains(" stop ") == true)
            #expect(command?.contains("muxy-notification-hook") == true)
        }
    }

    @Test("shipped muxy-grok-hook emits grok_hook agent_status and notification lines")
    func shippedHookEmitsGrokWireFormat() throws {
        let scriptPath = try #require(
            MuxyNotificationHooks.scriptPath(named: "muxy-grok-hook", extension: "sh")
                ?? Self.repositoryScriptPath()
        )
        let payloads = try Self.runHookScript(
            at: scriptPath,
            event: "stop",
            input: #"{}"#
        )
        #expect(!payloads.isEmpty)
        let joined = payloads.joined()
        #expect(joined.contains("agent_status|grok_hook|"))
        #expect(joined.contains("|idle"))
        #expect(joined.contains("grok_hook|"))
        #expect(joined.contains("|Grok|"))
        #expect(joined.contains("Session completed"))
        for payload in payloads {
            #expect(payload.hasSuffix("\n"))
        }
    }

    @Test("uninstall preserves foreign hooks in the shared file")
    func uninstallKeepsForeignEntriesOnDisk() throws {
        try withTempHome { home in
            let hooksDir = home.appendingPathComponent(".grok/hooks", isDirectory: true)
            try FileManager.default.createDirectory(at: hooksDir, withIntermediateDirectories: true)
            let hookURL = hooksDir.appendingPathComponent("muxy-notify.json")
            let preExisting: [String: Any] = [
                "hooks": [
                    "Stop": nonMuxyEntry(command: "echo foreign"),
                ],
            ]
            let data = try JSONSerialization.data(withJSONObject: preExisting, options: [.prettyPrinted, .sortedKeys])
            try data.write(to: hookURL)

            let script = home.appendingPathComponent("muxy-grok-hook.sh").path
            try "#!/bin/sh\n".write(toFile: script, atomically: true, encoding: .utf8)
            let provider = GrokProvider(homeDirectory: home.path, pathEnvironment: "")
            try provider.install(hookScriptPath: script)
            try provider.uninstall()

            #expect(FileManager.default.fileExists(atPath: hookURL.path))
            let remaining = try Data(contentsOf: hookURL)
            let json = try #require(JSONSerialization.jsonObject(with: remaining) as? [String: Any])
            let hooks = try #require(json["hooks"] as? [String: Any])
            #expect(hooks["Stop"] != nil)
            #expect(hooks["Notification"] == nil)
            let stop = try #require(hooks["Stop"] as? [[String: Any]])
            let command = (stop.first?["hooks"] as? [[String: Any]])?.first?["command"] as? String
            #expect(command == "echo foreign")
        }
    }

    @Test("isToolInstalled finds grok under injectable home paths")
    func detectsInstalledBinary() throws {
        try withTempHome { home in
            let bin = home.appendingPathComponent(".local/bin", isDirectory: true)
            try FileManager.default.createDirectory(at: bin, withIntermediateDirectories: true)
            let executable = bin.appendingPathComponent("grok")
            try "#!/bin/sh\n".write(to: executable, atomically: true, encoding: .utf8)
            try FileManager.default.setAttributes(
                [.posixPermissions: 0o755],
                ofItemAtPath: executable.path
            )
            let provider = GrokProvider(homeDirectory: home.path, pathEnvironment: "")
            #expect(provider.isToolInstalled())
        }
    }

    @Test("isToolInstalled is false when the binary is missing")
    func detectsMissingBinary() throws {
        try withTempHome { home in
            let provider = GrokProvider(homeDirectory: home.path, pathEnvironment: "")
            #expect(!provider.isToolInstalled())
        }
    }

    @Test("registry resolves grok_hook to the grok provider id and icon")
    @MainActor
    func registryResolvesGrok() {
        #expect(AIProviderRegistry.shared.notificationSource(for: "grok_hook") == .aiProvider("grok"))
        #expect(AIProviderRegistry.shared.iconName(for: .aiProvider("grok")) == "grok")
        #expect(AIProviderRegistry.shared.providers.contains(where: { $0.id == "grok" && $0.socketTypeKey == "grok_hook" }))
    }

    private func withTempHome(_ body: (URL) throws -> Void) throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("GrokProviderTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        try body(root)
    }

    private static func repositoryScriptPath() -> String? {
        let candidate = RepositoryRoot.find().appendingPathComponent("Muxy/Resources/scripts/muxy-grok-hook.sh")
        guard FileManager.default.fileExists(atPath: candidate.path) else { return nil }
        return candidate.path
    }

    private static func runHookScript(at scriptPath: String, event: String, input: String) throws -> [String] {
        let socketPath = "/tmp/mgh-\(ProcessInfo.processInfo.processIdentifier)-\(Int.random(in: 0 ..< 1_000_000)).sock"
        let listener = try bindListener(at: socketPath)
        defer {
            close(listener)
            unlink(socketPath)
        }

        let paneID = UUID().uuidString
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/bash")
        process.arguments = [scriptPath, event]
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
        stdin.fileHandleForWriting.write(Data(input.utf8))
        try stdin.fileHandleForWriting.close()

        let payloads = drainConnections(listener, while: process)
        let deadline = Date().addingTimeInterval(3)
        while process.isRunning, Date() < deadline {
            Thread.sleep(forTimeInterval: 0.01)
        }
        if process.isRunning {
            process.terminate()
            process.waitUntilExit()
        } else {
            process.waitUntilExit()
        }
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
}
