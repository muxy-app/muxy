import Foundation

public struct NotificationDTO: Identifiable, Codable, Sendable {
    public let id: UUID
    public let paneID: UUID
    public let projectID: UUID
    public let worktreeID: UUID
    public let areaID: UUID
    public let tabID: UUID
    public let source: SourceDTO
    public let title: String
    public let body: String
    public let timestamp: Date
    public var isRead: Bool

    public init(
        id: UUID,
        paneID: UUID,
        projectID: UUID,
        worktreeID: UUID,
        areaID: UUID,
        tabID: UUID,
        source: SourceDTO,
        title: String,
        body: String,
        timestamp: Date,
        isRead: Bool
    ) {
        self.id = id
        self.paneID = paneID
        self.projectID = projectID
        self.worktreeID = worktreeID
        self.areaID = areaID
        self.tabID = tabID
        self.source = source
        self.title = title
        self.body = body
        self.timestamp = timestamp
        self.isRead = isRead
    }

    public enum SourceDTO: Codable, Sendable, Equatable {
        case osc
        case aiProvider(String)
        case socket
    }
}
