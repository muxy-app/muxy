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
