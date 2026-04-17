import Foundation

public struct WorktreeDTO: Identifiable, Codable, Hashable, Sendable {
    public let id: UUID
    public var name: String
    public var path: String
    public var branch: String?
    public var isPrimary: Bool
    public var createdAt: Date

    public init(
        id: UUID,
        name: String,
        path: String,
        branch: String? = nil,
        isPrimary: Bool,
        createdAt: Date
    ) {
        self.id = id
        self.name = name
        self.path = path
        self.branch = branch
        self.isPrimary = isPrimary
        self.createdAt = createdAt
    }
}
