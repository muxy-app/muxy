import Foundation

enum WorkspaceType: String, Codable, Hashable {
    case local
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

struct RemoteProject: Identifiable, Codable, Hashable {
    let id: UUID
    var name: String
    var path: String
    var worktreesEnabled: Bool

    init(id: UUID = UUID(), name: String, path: String, worktreesEnabled: Bool = false) {
        self.id = id
        self.name = name
        self.path = path
        self.worktreesEnabled = worktreesEnabled
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(UUID.self, forKey: .id)
        name = try container.decode(String.self, forKey: .name)
        path = try container.decode(String.self, forKey: .path)
        worktreesEnabled = try container.decodeIfPresent(Bool.self, forKey: .worktreesEnabled) ?? false
    }

    func asProject(workspaceID: UUID, sortOrder: Int) -> Project {
        var project = Project(id: id, name: name, path: path, sortOrder: sortOrder, remoteWorkspaceID: workspaceID)
        project.worktreesEnabled = worktreesEnabled
        return project
    }
}

struct ProjectGroup: Identifiable, Codable, Hashable {
    let id: UUID
    var name: String
    var sortOrder: Int
    var projectIDs: [UUID]
    var type: WorkspaceType
    var sshData: SSHWorkspaceData?
    var remoteProjects: [RemoteProject]

    init(
        id: UUID = UUID(),
        name: String,
        sortOrder: Int = 0,
        projectIDs: [UUID] = [],
        type: WorkspaceType = .local,
        sshData: SSHWorkspaceData? = nil,
        remoteProjects: [RemoteProject] = []
    ) {
        self.id = id
        self.name = name
        self.sortOrder = sortOrder
        self.projectIDs = projectIDs
        self.type = type
        self.sshData = sshData
        self.remoteProjects = remoteProjects
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(UUID.self, forKey: .id)
        name = try container.decode(String.self, forKey: .name)
        sortOrder = try container.decode(Int.self, forKey: .sortOrder)
        projectIDs = try container.decodeIfPresent([UUID].self, forKey: .projectIDs) ?? []
        type = try container.decodeIfPresent(WorkspaceType.self, forKey: .type) ?? .local
        sshData = try container.decodeIfPresent(SSHWorkspaceData.self, forKey: .sshData)
        remoteProjects = try container.decodeIfPresent([RemoteProject].self, forKey: .remoteProjects) ?? []
    }

    var workspaceContext: WorkspaceContext {
        guard type == .ssh, let sshData else { return .local }
        return .ssh(sshData.destination)
    }

    var remoteHomeProject: Project? {
        guard type == .ssh, let sshData else { return nil }
        var project = Project(
            id: Self.remoteHomeID(for: id),
            name: Project.homeName,
            path: sshData.remoteRoot,
            sortOrder: Int.min,
            remoteWorkspaceID: id
        )
        project.icon = Project.homeIcon
        return project
    }

    static func remoteHomeID(for groupID: UUID) -> UUID {
        var bytes = groupID.uuid
        bytes.15 ^= 0x01
        return UUID(uuid: bytes)
    }
}
