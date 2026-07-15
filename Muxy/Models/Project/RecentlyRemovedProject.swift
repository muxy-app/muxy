import Foundation

struct RecentlyRemovedProject: Codable, Hashable, Identifiable {
    let project: Project
    let removedAt: Date

    var id: UUID { project.id }
}
