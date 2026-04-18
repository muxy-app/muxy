import Foundation

enum WorktreeSource: String, Codable, Hashable {
    case muxy
    case external
}

struct Worktree: Identifiable, Codable, Hashable {
    let id: UUID
    var name: String
    var path: String
    var branch: String?
    var ownsBranch: Bool
    var source: WorktreeSource
    var isPrimary: Bool
    var createdAt: Date

    init(
        id: UUID = UUID(),
        name: String,
        path: String,
        branch: String? = nil,
        ownsBranch: Bool = false,
        source: WorktreeSource = .muxy,
        isPrimary: Bool,
        createdAt: Date = Date()
    ) {
        self.id = id
        self.name = name
        self.path = path
        self.branch = branch
        self.ownsBranch = ownsBranch
        self.source = source
        self.isPrimary = isPrimary
        self.createdAt = createdAt
    }

    var isExternallyManaged: Bool {
        !isPrimary && source == .external
    }

    var canBeRemoved: Bool {
        !isPrimary && !isExternallyManaged
    }

    private enum CodingKeys: String, CodingKey {
        case id
        case name
        case path
        case branch
        case ownsBranch
        case source
        case isPrimary
        case createdAt
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(UUID.self, forKey: .id)
        name = try container.decode(String.self, forKey: .name)
        path = try container.decode(String.self, forKey: .path)
        branch = try container.decodeIfPresent(String.self, forKey: .branch)
        ownsBranch = try container.decodeIfPresent(Bool.self, forKey: .ownsBranch) ?? false
        source = try container.decodeIfPresent(WorktreeSource.self, forKey: .source) ?? .muxy
        isPrimary = try container.decode(Bool.self, forKey: .isPrimary)
        createdAt = try container.decodeIfPresent(Date.self, forKey: .createdAt) ?? Date()
    }
}
