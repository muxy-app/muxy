import AppKit
import Foundation
import Testing

@testable import Muxy

@Suite("BrowserAnnotationCacheStore")
@MainActor
struct BrowserAnnotationCacheStoreTests {
    @Test("writes a PNG and returns its URL")
    func writesPNG() throws {
        let store = BrowserAnnotationCacheStore.shared
        let id = UUID()
        defer { store.remove(id: id) }

        let image = makeImage(size: CGSize(width: 8, height: 8), color: .red)
        guard let url = store.write(image: image, id: id) else {
            Issue.record("write returned nil")
            return
        }
        #expect(FileManager.default.fileExists(atPath: url.path))
        #expect(url.lastPathComponent == "\(id.uuidString).png")
    }

    @Test("remove(id:) deletes the cached PNG")
    func removesPNG() throws {
        let store = BrowserAnnotationCacheStore.shared
        let id = UUID()
        let image = makeImage(size: CGSize(width: 4, height: 4), color: .blue)
        guard let url = store.write(image: image, id: id) else {
            Issue.record("write returned nil")
            return
        }
        store.remove(id: id)
        #expect(!FileManager.default.fileExists(atPath: url.path))
    }

    @Test("removeAll(olderThan:) deletes stale files but keeps fresh ones")
    func removesStaleFiles() throws {
        let store = BrowserAnnotationCacheStore.shared
        let staleID = UUID()
        let freshID = UUID()
        defer {
            store.remove(id: staleID)
            store.remove(id: freshID)
        }
        let image = makeImage(size: CGSize(width: 2, height: 2), color: .green)
        guard let staleURL = store.write(image: image, id: staleID),
              let freshURL = store.write(image: image, id: freshID)
        else {
            Issue.record("write returned nil")
            return
        }
        let oldDate = Date().addingTimeInterval(-3_600)
        try FileManager.default.setAttributes(
            [.modificationDate: oldDate],
            ofItemAtPath: staleURL.path
        )

        store.removeAll(olderThan: 60)

        #expect(!FileManager.default.fileExists(atPath: staleURL.path))
        #expect(FileManager.default.fileExists(atPath: freshURL.path))
    }

    private func makeImage(size: CGSize, color: NSColor) -> NSImage {
        let image = NSImage(size: size)
        image.lockFocus()
        color.setFill()
        NSRect(origin: .zero, size: size).fill()
        image.unlockFocus()
        return image
    }
}
