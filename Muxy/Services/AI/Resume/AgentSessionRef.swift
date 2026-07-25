import Foundation

struct AgentSessionRef: Equatable {
    let id: String
    let providerID: String
    let cwd: String
    let gitBranch: String?
    let title: String?
    let preview: String?
    let updatedAt: Date
    let pinned: Bool
    let archived: Bool
}
