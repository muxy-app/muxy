import Foundation

struct BrowserBookmark: Identifiable, Codable, Equatable {
    let id: UUID
    var title: String
    var url: String
    var createdAt: Date

    init(id: UUID = UUID(), title: String, url: String, createdAt: Date = Date()) {
        self.id = id
        self.title = title
        self.url = url
        self.createdAt = createdAt
    }
}

struct BrowserBookmarkCollection: Codable, Equatable {
    var bookmarks: [BrowserBookmark]

    init(bookmarks: [BrowserBookmark] = []) {
        self.bookmarks = bookmarks
    }
}
