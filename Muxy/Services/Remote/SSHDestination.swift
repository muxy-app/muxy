import Foundation

struct SSHConnectionKey: Hashable {
    let host: String
    let port: Int?
    let user: String?
    let identityFile: String?
}

struct SSHDestination: Hashable, Codable {
    var host: String
    var remoteRoot: String
    var port: Int?
    var user: String?
    var identityFile: String?
    var environment: [String: String]

    var connectionKey: SSHConnectionKey {
        SSHConnectionKey(host: host, port: port, user: user, identityFile: identityFile)
    }

    init(
        host: String,
        remoteRoot: String = "~",
        port: Int? = nil,
        user: String? = nil,
        identityFile: String? = nil,
        environment: [String: String] = SSHEnvironmentVariables.default
    ) {
        self.host = SSHFieldSanitizer.host(host)
        self.remoteRoot = SSHFieldSanitizer.root(remoteRoot)
        self.port = port
        self.user = SSHFieldSanitizer.optionalArgument(user)
        self.identityFile = SSHFieldSanitizer.identityFile(identityFile)
        self.environment = SSHEnvironmentVariables.sanitize(environment)
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        host = try SSHFieldSanitizer.host(container.decode(String.self, forKey: .host))
        remoteRoot = try SSHFieldSanitizer.root(container.decodeIfPresent(String.self, forKey: .remoteRoot))
        port = try container.decodeIfPresent(Int.self, forKey: .port)
        user = try SSHFieldSanitizer.optionalArgument(container.decodeIfPresent(String.self, forKey: .user))
        identityFile = try SSHFieldSanitizer.identityFile(container.decodeIfPresent(String.self, forKey: .identityFile))
        environment = try SSHEnvironmentVariables.defaulting(container.decodeIfPresent([String: String].self, forKey: .environment))
    }

    static func isValidHost(_ host: String) -> Bool {
        let trimmed = host.trimmingCharacters(in: .whitespacesAndNewlines)
        return !trimmed.isEmpty && !trimmed.hasPrefix("-")
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
