import Foundation

@MainActor
@Observable
final class MuxyNotification: Identifiable {
    enum Source {
        case osc
        case claudeHook
        case socket
        case vcs
    }

    let id = UUID()
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
}
