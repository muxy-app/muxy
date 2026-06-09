import Foundation
import NIOSSH

enum NativeSSHHostKeyValidation: Equatable {
    case trusted
    case changed
    case unknown
}

enum NativeSSHKnownHosts {
    static func validate(
        host: String,
        port: Int,
        hostKey: NIOSSHPublicKey,
        knownHosts: String
    ) -> NativeSSHHostKeyValidation {
        let matchedKeys = knownHosts
            .split(separator: "\n", omittingEmptySubsequences: false)
            .compactMap { entry(line: String($0), host: host, port: port) }

        guard !matchedKeys.isEmpty else { return .unknown }
        return matchedKeys.contains(hostKey) ? .trusted : .changed
    }

    static func loadDefaultKnownHosts() -> String {
        let url = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".ssh/known_hosts")
        return (try? String(contentsOf: url, encoding: .utf8)) ?? ""
    }

    private static func entry(line: String, host: String, port: Int) -> NIOSSHPublicKey? {
        let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, !trimmed.hasPrefix("#") else { return nil }

        var fields = trimmed.split(separator: " ", omittingEmptySubsequences: true).map(String.init)
        guard fields.first?.hasPrefix("@") == true || fields.count >= 3 else { return nil }
        if fields.first?.hasPrefix("@") == true {
            fields.removeFirst()
        }
        guard fields.count >= 3 else { return nil }
        guard hostPatterns(fields[0], match: host, port: port) else { return nil }
        return try? NIOSSHPublicKey(openSSHPublicKey: "\(fields[1]) \(fields[2])")
    }

    private static func hostPatterns(_ rawPatterns: String, match host: String, port: Int) -> Bool {
        let candidates = if port == 22 {
            [host]
        } else {
            [host, "[\(host)]:\(port)"]
        }
        return rawPatterns
            .split(separator: ",")
            .map(String.init)
            .contains { pattern in
                guard !pattern.hasPrefix("|") else { return false }
                return candidates.contains(pattern)
            }
    }
}
