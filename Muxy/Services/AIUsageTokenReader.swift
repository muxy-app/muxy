import Foundation

enum AIUsageTokenReader {
    static func fromEnvironment(
        keys: [String],
        env: [String: String] = ProcessInfo.processInfo.environment
    ) -> String? {
        for key in keys {
            if let value = env[key]?.trimmingCharacters(in: .whitespacesAndNewlines), !value.isEmpty {
                return value
            }
        }
        return nil
    }

    static func fromJSONFile(path: String, keys: [String]) throws -> String? {
        guard FileManager.default.fileExists(atPath: path) else { return nil }
        let data = try Data(contentsOf: URL(fileURLWithPath: path))
        guard let payload = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return nil
        }
        return AIUsageParserSupport.string(in: payload, keys: keys)
    }

    static func fromJSONFile(path: String, nestedKeyPath: [String], valueKeys: [String]) throws -> String? {
        guard FileManager.default.fileExists(atPath: path) else { return nil }
        let data = try Data(contentsOf: URL(fileURLWithPath: path))
        var current: Any? = try JSONSerialization.jsonObject(with: data)
        for key in nestedKeyPath {
            guard let dict = current as? [String: Any] else { return nil }
            current = dict[key]
        }
        guard let dict = current as? [String: Any] else { return nil }
        return AIUsageParserSupport.string(in: dict, keys: valueKeys)
    }

    static func fromKeychain(service: String, account: String? = nil) -> String? {
        var arguments = ["find-generic-password"]
        if let account, !account.isEmpty {
            arguments += ["-a", account]
        }
        arguments += ["-s", service, "-w"]

        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/security")
        process.arguments = arguments

        let stdout = Pipe()
        let stderr = Pipe()
        process.standardOutput = stdout
        process.standardError = stderr

        do {
            try process.run()
        } catch {
            return nil
        }

        let watcher = ProcessTimeoutWatcher.install(on: process, timeout: 5)
        let outputData = stdout.fileHandleForReading.readDataToEndOfFile()
        _ = stderr.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        watcher.cancel()

        guard process.terminationStatus == 0,
              let output = String(data: outputData, encoding: .utf8)
        else {
            return nil
        }

        let trimmed = output.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }
}
