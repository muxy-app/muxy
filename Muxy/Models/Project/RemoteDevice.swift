import Foundation

enum RemoteDeviceKind: String, Codable, Hashable {
    case ssh
}

enum SSHRemoteSessionMode: String, Codable, Hashable, CaseIterable {
    case direct
    case tmux
}

struct SSHWorkspaceData: Codable, Hashable {
    var host: String
    var remoteRoot: String
    var port: Int?
    var user: String?
    var identityFile: String?
    var environment: [String: String]
    var remoteSessionMode: SSHRemoteSessionMode

    init(
        host: String,
        remoteRoot: String = "~",
        port: Int? = nil,
        user: String? = nil,
        identityFile: String? = nil,
        environment: [String: String] = SSHEnvironmentVariables.default,
        remoteSessionMode: SSHRemoteSessionMode = .direct
    ) {
        self.host = SSHFieldSanitizer.host(host)
        self.remoteRoot = SSHFieldSanitizer.root(remoteRoot)
        self.port = port
        self.user = SSHFieldSanitizer.optionalArgument(user)
        self.identityFile = SSHFieldSanitizer.identityFile(identityFile)
        self.environment = SSHEnvironmentVariables.sanitize(environment)
        self.remoteSessionMode = remoteSessionMode
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        host = try SSHFieldSanitizer.host(container.decode(String.self, forKey: .host))
        remoteRoot = try SSHFieldSanitizer.root(container.decodeIfPresent(String.self, forKey: .remoteRoot))
        port = try container.decodeIfPresent(Int.self, forKey: .port)
        user = try SSHFieldSanitizer.optionalArgument(container.decodeIfPresent(String.self, forKey: .user))
        identityFile = try SSHFieldSanitizer.identityFile(container.decodeIfPresent(String.self, forKey: .identityFile))
        environment = try SSHEnvironmentVariables.defaulting(container.decodeIfPresent([String: String].self, forKey: .environment))
        remoteSessionMode = try container.decodeIfPresent(SSHRemoteSessionMode.self, forKey: .remoteSessionMode) ?? .direct
    }

    var destination: SSHDestination {
        SSHDestination(
            host: host,
            remoteRoot: remoteRoot,
            port: port,
            user: user,
            identityFile: identityFile,
            environment: environment,
            remoteSessionMode: remoteSessionMode
        )
    }
}

private extension String {
    var nilIfEmpty: String? { isEmpty ? nil : self }
}

struct RemoteDevice: Identifiable, Codable, Hashable {
    let id: UUID
    var name: String
    var kind: RemoteDeviceKind
    var ssh: SSHWorkspaceData

    init(id: UUID = UUID(), name: String, kind: RemoteDeviceKind = .ssh, ssh: SSHWorkspaceData) {
        self.id = id
        self.name = name
        self.kind = kind
        self.ssh = ssh
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(UUID.self, forKey: .id)
        name = try container.decode(String.self, forKey: .name)
        kind = try container.decodeIfPresent(RemoteDeviceKind.self, forKey: .kind) ?? .ssh
        ssh = try container.decode(SSHWorkspaceData.self, forKey: .ssh)
    }

    var destination: SSHDestination { ssh.destination }

    var displayName: String {
        name.trimmingCharacters(in: .whitespacesAndNewlines).nilIfEmpty ?? ssh.host
    }
}
