import Foundation

enum RemoteDeviceKind: String, Codable, Hashable {
    case ssh
    case muxy
}

struct MuxyRemoteServerData: Codable, Hashable {
    var host: String
    var port: UInt16
    var serviceName: String?

    init(host: String, port: UInt16 = 4865, serviceName: String? = nil) {
        self.host = Self.sanitizedHost(host)
        self.port = port
        self.serviceName = serviceName?.trimmingCharacters(in: .whitespacesAndNewlines).nilIfEmpty
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        host = try Self.sanitizedHost(container.decode(String.self, forKey: .host))
        port = try container.decodeIfPresent(UInt16.self, forKey: .port) ?? 4865
        serviceName = try container.decodeIfPresent(String.self, forKey: .serviceName)?
            .trimmingCharacters(in: .whitespacesAndNewlines).nilIfEmpty
    }

    var displayAddress: String { "\(host):\(port)" }

    var credentialScope: String { "\(host.lowercased()):\(port)" }

    var webSocketURL: URL? {
        var components = URLComponents()
        components.scheme = "ws"
        components.host = host
        components.port = Int(port)
        return components.url
    }

    static func isValidHost(_ host: String) -> Bool {
        !sanitizedHost(host).isEmpty
    }

    private static func sanitizedHost(_ value: String) -> String {
        value.trimmingCharacters(in: .whitespacesAndNewlines)
    }
}

struct SSHWorkspaceData: Codable, Hashable {
    var host: String
    var remoteRoot: String
    var port: Int?
    var user: String?
    var identityFile: String?
    var environment: [String: String]

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

    var destination: SSHDestination {
        SSHDestination(
            host: host,
            remoteRoot: remoteRoot,
            port: port,
            user: user,
            identityFile: identityFile,
            environment: environment
        )
    }
}

private extension String {
    var nilIfEmpty: String? { isEmpty ? nil : self }
}

enum RemoteDeviceConnection: Hashable {
    case ssh(SSHWorkspaceData)
    case muxy(MuxyRemoteServerData)
}

struct RemoteDevice: Identifiable, Codable, Hashable {
    let id: UUID
    var name: String
    private var connection: RemoteDeviceConnection

    init(id: UUID = UUID(), name: String, kind: RemoteDeviceKind = .ssh, ssh: SSHWorkspaceData) {
        precondition(kind == .ssh)
        self.id = id
        self.name = name
        connection = .ssh(ssh)
    }

    init(id: UUID = UUID(), name: String, muxy: MuxyRemoteServerData) {
        self.id = id
        self.name = name
        connection = .muxy(muxy)
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(UUID.self, forKey: .id)
        name = try container.decode(String.self, forKey: .name)
        let kind = try container.decodeIfPresent(RemoteDeviceKind.self, forKey: .kind) ?? .ssh
        switch kind {
        case .ssh:
            connection = try .ssh(container.decode(SSHWorkspaceData.self, forKey: .ssh))
        case .muxy:
            connection = try .muxy(container.decode(MuxyRemoteServerData.self, forKey: .muxy))
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(id, forKey: .id)
        try container.encode(name, forKey: .name)
        try container.encode(kind, forKey: .kind)
        switch connection {
        case let .ssh(data):
            try container.encode(data, forKey: .ssh)
        case let .muxy(data):
            try container.encode(data, forKey: .muxy)
        }
    }

    var kind: RemoteDeviceKind {
        switch connection {
        case .ssh: .ssh
        case .muxy: .muxy
        }
    }

    var ssh: SSHWorkspaceData {
        get {
            guard case let .ssh(data) = connection else { preconditionFailure("Device is not SSH") }
            return data
        }
        set { connection = .ssh(newValue) }
    }

    var muxy: MuxyRemoteServerData? {
        get {
            guard case let .muxy(data) = connection else { return nil }
            return data
        }
        set {
            guard let newValue else { return }
            connection = .muxy(newValue)
        }
    }

    var destination: SSHDestination { ssh.destination }

    var displayName: String {
        if let name = name.trimmingCharacters(in: .whitespacesAndNewlines).nilIfEmpty { return name }
        switch connection {
        case let .ssh(data): return data.host
        case let .muxy(data): return data.serviceName ?? data.host
        }
    }

    private enum CodingKeys: String, CodingKey {
        case id
        case name
        case kind
        case ssh
        case muxy
    }
}
