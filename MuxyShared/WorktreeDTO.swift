import Foundation

public struct WorktreeDTO: Identifiable, Codable, Hashable, Sendable {
    public let id: UUID
    public var name: String
    public var path: String
    public var branch: String?
    public var isPrimary: Bool
    public var canBeRemoved: Bool
    public var createdAt: Date

    private enum CodingKeys: String, CodingKey {
        case id
        case name
        case path
        case branch
        case isPrimary
        case canBeRemoved
        case createdAt
    }

    public init(
        id: UUID,
        name: String,
        path: String,
        branch: String? = nil,
        isPrimary: Bool,
        canBeRemoved: Bool? = nil,
        createdAt: Date
    ) {
        self.id = id
        self.name = name
        self.path = path
        self.branch = branch
        self.isPrimary = isPrimary
        self.canBeRemoved = canBeRemoved ?? !isPrimary
        self.createdAt = createdAt
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(UUID.self, forKey: .id)
        name = try container.decode(String.self, forKey: .name)
        path = try container.decode(String.self, forKey: .path)
        branch = try container.decodeIfPresent(String.self, forKey: .branch)
        isPrimary = try container.decode(Bool.self, forKey: .isPrimary)
        canBeRemoved = try container.decodeIfPresent(Bool.self, forKey: .canBeRemoved) ?? !isPrimary
        createdAt = try container.decode(Date.self, forKey: .createdAt)
    }
}
