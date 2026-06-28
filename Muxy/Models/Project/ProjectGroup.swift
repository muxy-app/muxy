import Foundation

enum WorkspaceType: String, Codable, Hashable {
    case local
    case ssh
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
    var remoteDeviceID: UUID?
    var remoteProjects: [RemoteProject]
    var legacySSHData: SSHWorkspaceData?

    init(
        id: UUID = UUID(),
        name: String,
        sortOrder: Int = 0,
        projectIDs: [UUID] = [],
        type: WorkspaceType = .local,
        remoteDeviceID: UUID? = nil,
        remoteProjects: [RemoteProject] = []
    ) {
        self.id = id
        self.name = name
        self.sortOrder = sortOrder
        self.projectIDs = projectIDs
        self.type = type
        self.remoteDeviceID = remoteDeviceID
        self.remoteProjects = remoteProjects
        legacySSHData = nil
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(UUID.self, forKey: .id)
        name = try container.decode(String.self, forKey: .name)
        sortOrder = try container.decode(Int.self, forKey: .sortOrder)
        projectIDs = try container.decodeIfPresent([UUID].self, forKey: .projectIDs) ?? []
        type = try container.decodeIfPresent(WorkspaceType.self, forKey: .type) ?? .local
        remoteDeviceID = try container.decodeIfPresent(UUID.self, forKey: .remoteDeviceID)
        remoteProjects = try container.decodeIfPresent([RemoteProject].self, forKey: .remoteProjects) ?? []
        legacySSHData = try container.decodeIfPresent(SSHWorkspaceData.self, forKey: .sshData)
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(id, forKey: .id)
        try container.encode(name, forKey: .name)
        try container.encode(sortOrder, forKey: .sortOrder)
        try container.encode(projectIDs, forKey: .projectIDs)
        try container.encode(type, forKey: .type)
        try container.encodeIfPresent(remoteDeviceID, forKey: .remoteDeviceID)
        try container.encode(remoteProjects, forKey: .remoteProjects)
    }

    private enum CodingKeys: String, CodingKey {
        case id
        case name
        case sortOrder
        case projectIDs
        case type
        case remoteDeviceID
        case remoteProjects
        case sshData
    }

    func workspaceContext(device: RemoteDevice?) -> WorkspaceContext {
        guard type == .ssh, let device else { return .local }
        return .ssh(device.destination)
    }

    func remoteHomeProject(device: RemoteDevice?) -> Project? {
        guard type == .ssh, let device else { return nil }
        var project = Project(
            id: Self.remoteHomeID(for: id),
            name: Project.homeName,
            path: device.ssh.remoteRoot,
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
