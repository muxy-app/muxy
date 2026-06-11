import Foundation

enum SSHEnvironmentVariables {
    static let `default` = ["TERM": "xterm-256color"]

    static func defaulting(_ environment: [String: String]?) -> [String: String] {
        guard let environment else { return Self.default }
        return sanitize(environment)
    }

    static func sanitize(_ environment: [String: String]) -> [String: String] {
        var sanitized: [String: String] = [:]
        for (key, value) in environment {
            let trimmedKey = key.trimmingCharacters(in: .whitespacesAndNewlines)
            guard RemoteCommandBuilder.isValidEnvironmentKey(trimmedKey) else { continue }
            sanitized[trimmedKey] = value
        }
        return sanitized
    }

    static func merged(device: [String: String], command: [String: String]?) -> [String: String] {
        var merged = sanitize(device)
        for (key, value) in sanitize(command ?? [:]) {
            merged[key] = value
        }
        return merged
    }
}
