import Foundation

struct Project: Identifiable, Codable, Hashable {
    let id: UUID
    var name: String
    var path: String
    var sortOrder: Int
    var createdAt: Date
    var lastActiveAt: Date?
    var icon: String?
    var logo: String?
    var iconColor: String?
    var preferredWorktreeParentPath: String?
    var worktreesEnabled: Bool
    var isPinned: Bool
    var remoteWorkspaceID: UUID?
    var remoteDeviceID: UUID?

    init(
        id: UUID = UUID(),
        name: String,
        path: String,
        sortOrder: Int = 0,
        remoteWorkspaceID: UUID? = nil,
        remoteDeviceID: UUID? = nil
    ) {
        self.id = id
        self.name = name
        self.path = path
        self.sortOrder = sortOrder
        self.createdAt = Date()
        self.lastActiveAt = nil
        self.icon = nil
        self.logo = nil
        self.iconColor = nil
        self.preferredWorktreeParentPath = nil
        self.worktreesEnabled = false
        self.isPinned = false
        self.remoteWorkspaceID = remoteWorkspaceID
        self.remoteDeviceID = remoteDeviceID
    }

    var isRemote: Bool { remoteWorkspaceID != nil || remoteDeviceID != nil }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(UUID.self, forKey: .id)
        name = try container.decode(String.self, forKey: .name)
        path = try container.decode(String.self, forKey: .path)
        sortOrder = try container.decode(Int.self, forKey: .sortOrder)
        createdAt = try container.decode(Date.self, forKey: .createdAt)
        lastActiveAt = try container.decodeIfPresent(Date.self, forKey: .lastActiveAt)
        icon = try container.decodeIfPresent(String.self, forKey: .icon)
        logo = try container.decodeIfPresent(String.self, forKey: .logo)
        iconColor = try container.decodeIfPresent(String.self, forKey: .iconColor)
        preferredWorktreeParentPath = try container.decodeIfPresent(String.self, forKey: .preferredWorktreeParentPath)
        worktreesEnabled = try container.decodeIfPresent(Bool.self, forKey: .worktreesEnabled) ?? false
        isPinned = try container.decodeIfPresent(Bool.self, forKey: .isPinned) ?? false
        remoteWorkspaceID = try container.decodeIfPresent(UUID.self, forKey: .remoteWorkspaceID)
        remoteDeviceID = try container.decodeIfPresent(UUID.self, forKey: .remoteDeviceID)
    }

    var pathExists: Bool {
        guard !isRemote else { return true }
        return FileManager.default.fileExists(atPath: path)
    }

    var isHome: Bool {
        if id == Project.homeID { return true }
        guard let remoteWorkspaceID else { return false }
        return id == ProjectGroup.remoteHomeID(for: remoteWorkspaceID)
    }
}

extension Project {
    static let homeID = UUID(uuid: (0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1))
    static let homeName = "Home"
    static let homeIcon = "house.fill"

    static let home = Project(
        id: homeID,
        name: homeName,
        path: FileManager.default.homeDirectoryForCurrentUser.path,
        sortOrder: Int.min
    )
}
