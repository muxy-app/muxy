import Foundation

struct BrowserHistoryEntry: Codable, Identifiable, Hashable {
    let id: UUID
    let profileID: UUID
    var url: String
    var title: String?
    var lastVisited: Date
    var visitCount: Int

    init(
        id: UUID = UUID(),
        profileID: UUID,
        url: String,
        title: String? = nil,
        lastVisited: Date,
        visitCount: Int = 1
    ) {
        self.id = id
        self.profileID = profileID
        self.url = url
        self.title = title
        self.lastVisited = lastVisited
        self.visitCount = visitCount
    }

    func matches(query: String) -> Bool {
        let needle = query.lowercased()
        if url.lowercased().contains(needle) {
            return true
        }
        guard let title else { return false }
        return title.lowercased().contains(needle)
    }
}
