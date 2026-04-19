import Foundation

public struct ProjectDTO: Identifiable, Codable, Hashable, Sendable {
    public let id: UUID
    public var name: String
    public var path: String
    public var sortOrder: Int
    public var createdAt: Date
    public var icon: String?
    public var logo: String?
    public var iconColor: String?

    public init(
        id: UUID,
        name: String,
        path: String,
        sortOrder: Int,
        createdAt: Date,
        icon: String? = nil,
        logo: String? = nil,
        iconColor: String? = nil
    ) {
        self.id = id
        self.name = name
        self.path = path
        self.sortOrder = sortOrder
        self.createdAt = createdAt
        self.icon = icon
        self.logo = logo
        self.iconColor = iconColor
    }
}
