import Foundation

struct Worktree: Identifiable, Codable, Hashable {
    let id: UUID
    var name: String
    var path: String
    var branch: String?
    var isPrimary: Bool
    var createdAt: Date

    init(
        id: UUID = UUID(),
        name: String,
        path: String,
        branch: String? = nil,
        isPrimary: Bool,
        createdAt: Date = Date()
    ) {
        self.id = id
        self.name = name
        self.path = path
        self.branch = branch
        self.isPrimary = isPrimary
        self.createdAt = createdAt
    }
}
