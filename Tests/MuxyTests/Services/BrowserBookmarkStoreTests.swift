import Foundation
import Testing

@testable import Muxy

@Suite("BrowserBookmarkStore")
@MainActor
struct BrowserBookmarkStoreTests {
    private func makeStore() -> (BrowserBookmarkStore, URL) {
        let fileURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("browser-bookmarks-\(UUID().uuidString).json")
        return (BrowserBookmarkStore(fileURL: fileURL), fileURL)
    }

    @Test("adds bookmark per project")
    func addBookmark() {
        let (store, fileURL) = makeStore()
        defer { try? FileManager.default.removeItem(at: fileURL) }

        let bookmark = BrowserBookmark(title: "Docs", url: "https://example.com")
        store.add(bookmark, projectPath: "/project")

        #expect(store.bookmarks(for: "/project") == [bookmark])
        #expect(store.bookmarks(for: "/other").isEmpty)
    }

    @Test("updating same url replaces title")
    func updateExistingBookmarkTitle() {
        let (store, fileURL) = makeStore()
        defer { try? FileManager.default.removeItem(at: fileURL) }

        let original = BrowserBookmark(title: "Old", url: "https://example.com")
        store.add(original, projectPath: "/project")
        let updated = BrowserBookmark(title: "New", url: "https://example.com")
        store.add(updated, projectPath: "/project")

        #expect(store.bookmarks(for: "/project").count == 1)
        #expect(store.bookmarks(for: "/project").first?.title == "New")
    }

    @Test("removes bookmark by id")
    func removesBookmark() {
        let (store, fileURL) = makeStore()
        defer { try? FileManager.default.removeItem(at: fileURL) }

        let bookmark = BrowserBookmark(title: "Docs", url: "https://example.com")
        store.add(bookmark, projectPath: "/project")
        store.remove(id: bookmark.id, projectPath: "/project")

        #expect(store.bookmarks(for: "/project").isEmpty)
    }

    @Test("persists bookmarks across store instances")
    func persistsBookmarks() {
        let fileURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("browser-bookmarks-\(UUID().uuidString).json")
        defer { try? FileManager.default.removeItem(at: fileURL) }

        let bookmark = BrowserBookmark(title: "Docs", url: "https://example.com")
        let firstStore = BrowserBookmarkStore(fileURL: fileURL)
        firstStore.add(bookmark, projectPath: "/project")

        let secondStore = BrowserBookmarkStore(fileURL: fileURL)
        #expect(secondStore.bookmarks(for: "/project") == [bookmark])
    }
}
