import CryptoKit
import Darwin
import Foundation
import NIOSSH

public enum SSHHostKeyValidation: Equatable {
    case trusted
    case changed
    case unknown
}

public enum SSHKnownHosts {
    public static func validate(
        host: String,
        port: Int,
        hostKey: NIOSSHPublicKey,
        knownHosts: String
    ) -> SSHHostKeyValidation {
        let matchedKeys = knownHosts
            .split(separator: "\n", omittingEmptySubsequences: false)
            .compactMap { entry(line: String($0), host: host, port: port) }

        guard !matchedKeys.isEmpty else { return .unknown }
        return matchedKeys.contains(hostKey) ? .trusted : .changed
    }

    public static func loadDefaultKnownHosts() -> String {
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
        var candidates = [host, "[\(host)]"]
        if port != 22 {
            candidates.append("[\(host)]:\(port)")
        }
        if port == 22 {
            candidates.append("[\(host)]:22")
        }
        return rawPatterns
            .split(separator: ",")
            .map(String.init)
            .contains { pattern in
                let isHashed = pattern.hasPrefix("|1|")
                if isHashed {
                    return candidates.contains { hashedHostMatch(pattern, candidate: $0) }
                }
                if pattern.contains("*") || pattern.contains("?") {
                    return candidates.contains { wildcardMatch(pattern: pattern, candidate: $0) }
                }
                return candidates.contains(pattern)
            }
    }

    private static func wildcardMatch(pattern: String, candidate: String) -> Bool {
        pattern.withCString { patternCString in
            candidate.withCString { candidateCString in
                fnmatch(patternCString, candidateCString, FNM_CASEFOLD) == 0
            }
        }
    }

    private static func hashedHostMatch(_ pattern: String, candidate: String) -> Bool {
        let components = pattern.split(separator: "|", omittingEmptySubsequences: false).map(String.init)
        guard components.count == 4, components[0].isEmpty, components[1] == "1" else { return false }

        guard let salt = Data(base64Encoded: components[2]),
              let expectedDigest = Data(base64Encoded: components[3]),
              let candidateData = candidate.data(using: .utf8)
        else {
            return false
        }

        let derived = HMAC<Insecure.SHA1>.authenticationCode(for: candidateData, using: SymmetricKey(data: salt))
        return Data(derived) == expectedDigest
    }
}
