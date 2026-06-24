import Foundation

protocol BrowserHistoryPersisting {
    func loadEntries() throws -> [BrowserHistoryEntry]
    func saveEntries(_ entries: [BrowserHistoryEntry]) throws
}

final class FileBrowserHistoryPersistence: BrowserHistoryPersisting {
    private let store: CodableFileStore<[BrowserHistoryEntry]>

    init(fileURL: URL = MuxyFileStorage.fileURL(filename: "browser-history.json")) {
        store = CodableFileStore(
            fileURL: fileURL,
            options: CodableFileStoreOptions(filePermissions: FilePermissions.privateFile)
        )
    }

    func loadEntries() throws -> [BrowserHistoryEntry] {
        try store.load() ?? []
    }

    func saveEntries(_ entries: [BrowserHistoryEntry]) throws {
        try store.save(entries)
    }
}

final class InMemoryBrowserHistoryPersistence: BrowserHistoryPersisting {
    private var entries: [BrowserHistoryEntry]

    init(initial: [BrowserHistoryEntry] = []) {
        entries = initial
    }

    func loadEntries() throws -> [BrowserHistoryEntry] {
        entries
    }

    func saveEntries(_ entries: [BrowserHistoryEntry]) throws {
        self.entries = entries
    }
}
