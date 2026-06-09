import Foundation

struct Project: Identifiable, Codable, Hashable {
    let id: UUID
    var name: String
    var path: String
    var sortOrder: Int
    var createdAt: Date
    var icon: String?
    var logo: String?
    var iconColor: String?
    var preferredWorktreeParentPath: String?
    var worktreesEnabled: Bool

    init(id: UUID = UUID(), name: String, path: String, sortOrder: Int = 0) {
        self.id = id
        self.name = name
        self.path = path
        self.sortOrder = sortOrder
        self.createdAt = Date()
        self.icon = nil
        self.logo = nil
        self.iconColor = nil
        self.preferredWorktreeParentPath = nil
        self.worktreesEnabled = false
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(UUID.self, forKey: .id)
        name = try container.decode(String.self, forKey: .name)
        path = try container.decode(String.self, forKey: .path)
        sortOrder = try container.decode(Int.self, forKey: .sortOrder)
        createdAt = try container.decode(Date.self, forKey: .createdAt)
        icon = try container.decodeIfPresent(String.self, forKey: .icon)
        logo = try container.decodeIfPresent(String.self, forKey: .logo)
        iconColor = try container.decodeIfPresent(String.self, forKey: .iconColor)
        preferredWorktreeParentPath = try container.decodeIfPresent(String.self, forKey: .preferredWorktreeParentPath)
        worktreesEnabled = try container.decodeIfPresent(Bool.self, forKey: .worktreesEnabled) ?? false
    }

    var pathExists: Bool {
        FileManager.default.fileExists(atPath: path)
    }

    var isHome: Bool {
        id == Project.homeID
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
