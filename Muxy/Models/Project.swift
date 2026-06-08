import Foundation

struct Project: Identifiable, Codable, Hashable {
    enum Kind: Codable, Equatable, Hashable {
        case local
        case remote(RemoteProjectConfig)
    }

    let id: UUID
    var name: String
    var path: String
    var kind: Kind = .local
    var sortOrder: Int
    var createdAt: Date
    var icon: String?
    var logo: String?
    var iconColor: String?
    var preferredWorktreeParentPath: String?

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(UUID.self, forKey: .id)
        name = try container.decode(String.self, forKey: .name)
        path = try container.decode(String.self, forKey: .path)
        kind = try container.decodeIfPresent(Kind.self, forKey: .kind) ?? .local
        sortOrder = try container.decode(Int.self, forKey: .sortOrder)
        createdAt = try container.decode(Date.self, forKey: .createdAt)
        icon = try container.decodeIfPresent(String.self, forKey: .icon)
        logo = try container.decodeIfPresent(String.self, forKey: .logo)
        iconColor = try container.decodeIfPresent(String.self, forKey: .iconColor)
        preferredWorktreeParentPath = try container.decodeIfPresent(String.self, forKey: .preferredWorktreeParentPath)
    }

    init(id: UUID = UUID(), name: String, path: String, sortOrder: Int = 0) {
        self.id = id
        self.name = name
        self.path = path
        self.kind = .local
        self.sortOrder = sortOrder
        self.createdAt = Date()
        self.icon = nil
        self.logo = nil
        self.iconColor = nil
        self.preferredWorktreeParentPath = nil
    }

    init(id: UUID = UUID(), name: String, remoteConfig: RemoteProjectConfig) {
        self.id = id
        self.name = name
        self.path = Self.remoteProjectLocalPath(id: id)
        self.kind = .remote(remoteConfig)
        self.sortOrder = 0
        self.createdAt = Date()
        self.icon = remoteConfig.icon
        self.logo = nil
        self.iconColor = remoteConfig.iconColor
        self.preferredWorktreeParentPath = nil
    }

    var isRemote: Bool {
        if case .remote = kind { return true }
        return false
    }

    var remoteConfig: RemoteProjectConfig? {
        if case let .remote(config) = kind { return config }
        return nil
    }

    var pathExists: Bool {
        if isRemote { return true }
        return FileManager.default.fileExists(atPath: path)
    }

    var isHome: Bool {
        id == Project.homeID
    }

    static func remoteProjectLocalPath(id: UUID) -> String {
        let base = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".config/muxy/remote-projects")
            .appendingPathComponent(id.uuidString)
        return base.path
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
