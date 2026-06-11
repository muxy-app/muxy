import Foundation

struct SSHDestination: Hashable, Codable {
    var host: String
    var remoteRoot: String
    var port: Int?
    var user: String?
    var identityFile: String?

    init(
        host: String,
        remoteRoot: String = "~",
        port: Int? = nil,
        user: String? = nil,
        identityFile: String? = nil
    ) {
        self.host = Self.sanitizedHost(host)
        let trimmedRoot = remoteRoot.trimmingCharacters(in: .whitespacesAndNewlines)
        self.remoteRoot = trimmedRoot.isEmpty ? "~" : trimmedRoot
        self.port = port
        self.user = Self.sanitizedUser(user)
        self.identityFile = identityFile.flatMap { $0.isEmpty ? nil : $0 }
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        host = try Self.sanitizedHost(container.decode(String.self, forKey: .host))
        remoteRoot = try container.decodeIfPresent(String.self, forKey: .remoteRoot) ?? "~"
        port = try container.decodeIfPresent(Int.self, forKey: .port)
        user = try Self.sanitizedUser(container.decodeIfPresent(String.self, forKey: .user))
        identityFile = try container.decodeIfPresent(String.self, forKey: .identityFile)
    }

    static func isValidHost(_ host: String) -> Bool {
        let trimmed = host.trimmingCharacters(in: .whitespacesAndNewlines)
        return !trimmed.isEmpty && !trimmed.hasPrefix("-")
    }

    private static func sanitizedHost(_ host: String) -> String {
        let trimmed = host.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.hasPrefix("-") ? String(trimmed.drop { $0 == "-" }) : trimmed
    }

    private static func sanitizedUser(_ user: String?) -> String? {
        guard let trimmed = user?.trimmingCharacters(in: .whitespacesAndNewlines), !trimmed.isEmpty else {
            return nil
        }
        return trimmed.hasPrefix("-") ? String(trimmed.drop { $0 == "-" }) : trimmed
    }

    var target: String {
        guard let user else { return host }
        return "\(user)@\(host)"
    }

    var connectionArguments: [String] {
        var arguments: [String] = []
        if let port {
            arguments += ["-p", String(port)]
        }
        if let identityFile {
            arguments += ["-i", identityFile, "-o", "IdentitiesOnly=yes"]
        }
        return arguments
    }

    private static let keepAliveOptions: [String] = [
        "-o", "ConnectTimeout=8",
        "-o", "ServerAliveInterval=15",
        "-o", "ServerAliveCountMax=3",
    ]

    private static let nonInteractiveOptions: [String] = [
        "-o", "BatchMode=yes",
        "-o", "StrictHostKeyChecking=accept-new",
    ]

    private static let interactiveOptions: [String] = [
        "-o", "StrictHostKeyChecking=accept-new",
    ]

    private static let multiplexOptions: [String] = [
        "-o", "ControlMaster=auto",
        "-o", "ControlPath=~/.ssh/muxy-%C",
        "-o", "ControlPersist=120",
    ]

    static let batchOptions: [String] = nonInteractiveOptions + multiplexOptions + keepAliveOptions

    static let connectOptions: [String] = nonInteractiveOptions + multiplexOptions + keepAliveOptions

    static let terminalOptions: [String] = ["-o", "ControlMaster=no"] + interactiveOptions + keepAliveOptions
}
