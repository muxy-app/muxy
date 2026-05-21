import Foundation

struct ProjectGroup: Identifiable, Codable, Hashable {
    let id: UUID
    var name: String
    var sortOrder: Int
    var projectIDs: [UUID]

    init(name: String, sortOrder: Int = 0, projectIDs: [UUID] = []) {
        self.id = UUID()
        self.name = name
        self.sortOrder = sortOrder
        self.projectIDs = projectIDs
    }
}
