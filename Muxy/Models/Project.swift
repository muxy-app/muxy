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
    var isNameCustomized: Bool

    init(name: String, path: String, sortOrder: Int = 0, isNameCustomized: Bool = false) {
        self.id = UUID()
        self.name = name
        self.path = path
        self.sortOrder = sortOrder
        self.createdAt = Date()
        self.icon = nil
        self.logo = nil
        self.iconColor = nil
        self.isNameCustomized = isNameCustomized
    }

    private enum CodingKeys: String, CodingKey {
        case id, name, path, sortOrder, createdAt, icon, logo, iconColor, isNameCustomized
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
        let stored = try container.decodeIfPresent(Bool.self, forKey: .isNameCustomized)
        let folderName = URL(fileURLWithPath: path).lastPathComponent
        isNameCustomized = stored ?? (name != folderName)
    }

    var pathExists: Bool {
        FileManager.default.fileExists(atPath: path)
    }
}
