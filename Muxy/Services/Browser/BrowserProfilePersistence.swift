import Foundation

protocol BrowserProfilePersisting {
    func loadProfiles() throws -> [BrowserProfile]
    func saveProfiles(_ profiles: [BrowserProfile]) throws
}

final class FileBrowserProfilePersistence: BrowserProfilePersisting {
    private let store: CodableFileStore<[BrowserProfile]>

    init(fileURL: URL = MuxyFileStorage.fileURL(filename: "browser-profiles.json")) {
        store = CodableFileStore(
            fileURL: fileURL,
            options: CodableFileStoreOptions(filePermissions: FilePermissions.privateFile)
        )
    }

    func loadProfiles() throws -> [BrowserProfile] {
        try store.load() ?? []
    }

    func saveProfiles(_ profiles: [BrowserProfile]) throws {
        try store.save(profiles)
    }
}

final class InMemoryBrowserProfilePersistence: BrowserProfilePersisting {
    private var profiles: [BrowserProfile]

    init(initial: [BrowserProfile] = []) {
        profiles = initial
    }

    func loadProfiles() throws -> [BrowserProfile] {
        profiles
    }

    func saveProfiles(_ profiles: [BrowserProfile]) throws {
        self.profiles = profiles
    }
}
