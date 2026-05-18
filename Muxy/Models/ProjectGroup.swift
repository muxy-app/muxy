import Foundation

struct ProjectGroup: Identifiable, Codable, Hashable {
    let id: UUID
    var name: String
    var sortOrder: Int
    var isExpanded: Bool
    var projectIDs: [UUID]

    init(name: String, sortOrder: Int = 0, isExpanded: Bool = true, projectIDs: [UUID] = []) {
        self.id = UUID()
        self.name = name
        self.sortOrder = sortOrder
        self.isExpanded = isExpanded
        self.projectIDs = projectIDs
    }
}
