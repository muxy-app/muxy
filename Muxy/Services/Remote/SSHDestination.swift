import Foundation

struct SSHConnectionKey: Hashable {
    let host: String
    let port: Int?
    let user: String?
    let identityFile: String?
    let authenticationMethod: SSHAuthenticationMethod
}

struct SSHDestination: Hashable, Codable {
    var host: String
    var remoteRoot: String
    var port: Int?
    var user: String?
    var identityFile: String?
    var authenticationMethod: SSHAuthenticationMethod
    var environment: [String: String]

    var connectionKey: SSHConnectionKey {
        SSHConnectionKey(
            host: host,
            port: port,
            user: user,
            identityFile: identityFile,
            authenticationMethod: authenticationMethod
        )
    }

    init(
        host: String,
        remoteRoot: String = "~",
        port: Int? = nil,
        user: String? = nil,
        identityFile: String? = nil,
        authenticationMethod: SSHAuthenticationMethod? = nil,
        environment: [String: String] = SSHEnvironmentVariables.default
    ) {
        self.host = SSHFieldSanitizer.host(host)
        self.remoteRoot = SSHFieldSanitizer.root(remoteRoot)
        self.port = port
        self.user = SSHFieldSanitizer.optionalArgument(user)
        self.identityFile = SSHFieldSanitizer.identityFile(identityFile)
        self.authenticationMethod = authenticationMethod ?? (self.identityFile == nil ? .automatic : .privateKey)
        self.environment = SSHEnvironmentVariables.sanitize(environment)
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        host = try SSHFieldSanitizer.host(container.decode(String.self, forKey: .host))
        remoteRoot = try SSHFieldSanitizer.root(container.decodeIfPresent(String.self, forKey: .remoteRoot))
        port = try container.decodeIfPresent(Int.self, forKey: .port)
        user = try SSHFieldSanitizer.optionalArgument(container.decodeIfPresent(String.self, forKey: .user))
        identityFile = try SSHFieldSanitizer.identityFile(container.decodeIfPresent(String.self, forKey: .identityFile))
        authenticationMethod = try container.decodeIfPresent(
            SSHAuthenticationMethod.self,
            forKey: .authenticationMethod
        ) ?? (identityFile == nil ? .automatic : .privateKey)
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
}
