import Foundation

struct WorktreeRecord: Hashable {
    let name: String
    let path: String
    let branch: String?
    let head: String?
    let isBare: Bool
    let isDetached: Bool
    let isPrunable: Bool

    init(
        name: String = "",
        path: String,
        branch: String?,
        head: String?,
        isBare: Bool,
        isDetached: Bool,
        isPrunable: Bool = false
    ) {
        self.name = name
        self.path = path
        self.branch = branch
        self.head = head
        self.isBare = isBare
        self.isDetached = isDetached
        self.isPrunable = isPrunable
    }
}
