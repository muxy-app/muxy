import Foundation

@MainActor
final class MuxyNotification: Identifiable, @preconcurrency Codable {
    enum Source: Equatable, Codable {
        case osc
        case aiProvider(String)
        case socket
    }

    let id: UUID
    let paneID: UUID
    let projectID: UUID
    let worktreeID: UUID
    let areaID: UUID
    let tabID: UUID
    let worktreePath: String
    let source: Source
    let title: String
    let body: String
    let timestamp: Date
    var isRead: Bool

    init(
        paneID: UUID,
        projectID: UUID,
        worktreeID: UUID,
        areaID: UUID,
        tabID: UUID,
        worktreePath: String,
        source: Source,
        title: String,
        body: String,
        isRead: Bool = false
    ) {
        id = UUID()
        self.paneID = paneID
        self.projectID = projectID
        self.worktreeID = worktreeID
        self.areaID = areaID
        self.tabID = tabID
        self.worktreePath = worktreePath
        self.source = source
        self.title = title
        self.body = body
        timestamp = Date()
        self.isRead = isRead
    }

    enum CodingKeys: String, CodingKey {
        case id
        case paneID
        case projectID
        case worktreeID
        case areaID
        case tabID
        case worktreePath
        case source
        case title
        case body
        case timestamp
        case isRead
    }

    required init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(UUID.self, forKey: .id)
        paneID = try container.decode(UUID.self, forKey: .paneID)
        projectID = try container.decode(UUID.self, forKey: .projectID)
        worktreeID = try container.decode(UUID.self, forKey: .worktreeID)
        areaID = try container.decode(UUID.self, forKey: .areaID)
        tabID = try container.decode(UUID.self, forKey: .tabID)
        worktreePath = try container.decode(String.self, forKey: .worktreePath)
        source = try container.decode(Source.self, forKey: .source)
        title = try container.decode(String.self, forKey: .title)
        body = try container.decode(String.self, forKey: .body)
        timestamp = try container.decode(Date.self, forKey: .timestamp)
        isRead = try container.decode(Bool.self, forKey: .isRead)
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(id, forKey: .id)
        try container.encode(paneID, forKey: .paneID)
        try container.encode(projectID, forKey: .projectID)
        try container.encode(worktreeID, forKey: .worktreeID)
        try container.encode(areaID, forKey: .areaID)
        try container.encode(tabID, forKey: .tabID)
        try container.encode(worktreePath, forKey: .worktreePath)
        try container.encode(source, forKey: .source)
        try container.encode(title, forKey: .title)
        try container.encode(body, forKey: .body)
        try container.encode(timestamp, forKey: .timestamp)
        try container.encode(isRead, forKey: .isRead)
    }
}
