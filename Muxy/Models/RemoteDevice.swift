import Foundation

enum RemoteDeviceKind: String, Codable, Hashable {
    case ssh
}

struct SSHWorkspaceData: Codable, Hashable {
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
        self.host = host.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedRoot = remoteRoot.trimmingCharacters(in: .whitespacesAndNewlines)
        self.remoteRoot = trimmedRoot.isEmpty ? "~" : trimmedRoot
        self.port = port
        self.user = user?.trimmingCharacters(in: .whitespacesAndNewlines).nilIfEmpty
        self.identityFile = identityFile?.trimmingCharacters(in: .whitespacesAndNewlines).nilIfEmpty
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        host = try container.decode(String.self, forKey: .host)
        remoteRoot = try container.decodeIfPresent(String.self, forKey: .remoteRoot) ?? "~"
        port = try container.decodeIfPresent(Int.self, forKey: .port)
        user = try container.decodeIfPresent(String.self, forKey: .user)
        identityFile = try container.decodeIfPresent(String.self, forKey: .identityFile)
    }

    var destination: SSHDestination {
        SSHDestination(host: host, remoteRoot: remoteRoot, port: port, user: user, identityFile: identityFile)
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
