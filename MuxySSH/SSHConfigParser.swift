import Foundation

public enum SSHConfigParser {
    public struct ParsedHost {
        public let name: String
        public let hostName: String
        public let user: String?
        public let port: UInt16
        public let identityFile: String?

        public init(name: String, hostName: String, user: String?, port: UInt16, identityFile: String?) {
            self.name = name
            self.hostName = hostName
            self.user = user
            self.port = port
            self.identityFile = identityFile
        }
    }

    public static func parse(configPath: String? = nil) -> [ParsedHost] {
        let path = configPath ?? FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".ssh/config").path

        guard let content = try? String(contentsOfFile: path, encoding: .utf8) else {
            return []
        }

        var hosts: [ParsedHost] = []
        var currentNames: [String] = []
        var currentHostName: String?
        var currentUser: String?
        var currentPort: UInt16 = 22
        var currentIdentityFile: String?

        let lines = content.components(separatedBy: .newlines)

        func flushCurrent() {
            for name in currentNames {
                guard !name.contains("*") else { continue }
                guard !name.contains("?") else { continue }
                guard let hostName = currentHostName else { continue }
                let host = ParsedHost(
                    name: name,
                    hostName: hostName,
                    user: currentUser,
                    port: currentPort,
                    identityFile: currentIdentityFile
                )
                hosts.append(host)
            }
        }

        for line in lines {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed.isEmpty || trimmed.hasPrefix("#") {
                continue
            }

            let parts = trimmed.components(separatedBy: .whitespaces)
            guard let directive = parts.first?.lowercased(), parts.count >= 2 else {
                continue
            }

            let value = parts.dropFirst().joined(separator: " ")

            switch directive {
            case "host":
                flushCurrent()
                currentNames = parts.dropFirst().map(\.self)
                currentHostName = nil
                currentUser = nil
                currentPort = 22
                currentIdentityFile = nil
            case "hostname":
                currentHostName = value
            case "user":
                currentUser = value
            case "port":
                currentPort = UInt16(value) ?? 22
            case "identityfile":
                currentIdentityFile = (value as NSString).expandingTildeInPath
            default:
                break
            }
        }

        flushCurrent()

        return hosts.filter { $0.name != "*" }
    }
}
