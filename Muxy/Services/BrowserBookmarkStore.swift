import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "BrowserBookmarkStore")

@MainActor
@Observable
final class BrowserBookmarkStore {
    static let shared = BrowserBookmarkStore()

    private(set) var bookmarksByProjectPath: [String: [BrowserBookmark]] = [:]

    private let fileStore: CodableFileStore<[String: BrowserBookmarkCollection]>

    init(fileURL: URL = MuxyFileStorage.fileURL(filename: "browser-bookmarks.json")) {
        fileStore = CodableFileStore(fileURL: fileURL, options: .prettySorted)
        load()
    }

    func bookmarks(for projectPath: String) -> [BrowserBookmark] {
        bookmarksByProjectPath[projectPath, default: []]
    }

    func add(_ bookmark: BrowserBookmark, projectPath: String) {
        var list = bookmarksByProjectPath[projectPath, default: []]
        if let index = list.firstIndex(where: { $0.url == bookmark.url }) {
            list[index].title = bookmark.title
        } else {
            list.append(bookmark)
        }
        bookmarksByProjectPath[projectPath] = list
        save()
    }

    func remove(id: UUID, projectPath: String) {
        guard var list = bookmarksByProjectPath[projectPath] else { return }
        list.removeAll { $0.id == id }
        bookmarksByProjectPath[projectPath] = list
        save()
    }

    private func load() {
        do {
            guard let stored = try fileStore.load() else { return }
            bookmarksByProjectPath = stored.mapValues(\.bookmarks)
        } catch {
            logger.error("Failed to load browser bookmarks: \(error.localizedDescription)")
        }
    }

    private func save() {
        let mapped = bookmarksByProjectPath.mapValues { BrowserBookmarkCollection(bookmarks: $0) }
        do {
            try fileStore.save(mapped)
        } catch {
            logger.error("Failed to save browser bookmarks: \(error.localizedDescription)")
        }
    }
}
