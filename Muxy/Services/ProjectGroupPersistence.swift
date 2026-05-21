import Foundation

protocol ProjectGroupPersisting {
    func loadProjectGroups() throws -> [ProjectGroup]
    func saveProjectGroups(_ groups: [ProjectGroup]) throws
    func loadActiveGroupID() -> UUID?
    func saveActiveGroupID(_ id: UUID?)
}

final class FileProjectGroupPersistence: ProjectGroupPersisting {
    private let store: CodableFileStore<[ProjectGroup]>
    private let defaults: UserDefaults
    private let activeGroupKey: String

    init(
        fileURL: URL = MuxyFileStorage.fileURL(filename: "project-groups.json"),
        defaults: UserDefaults = .standard,
        activeGroupKey: String = "muxy.activeProjectGroupID"
    ) {
        store = CodableFileStore(fileURL: fileURL)
        self.defaults = defaults
        self.activeGroupKey = activeGroupKey
    }

    func loadProjectGroups() throws -> [ProjectGroup] {
        try store.load() ?? []
    }

    func saveProjectGroups(_ groups: [ProjectGroup]) throws {
        try store.save(groups)
    }

    func loadActiveGroupID() -> UUID? {
        guard let idString = defaults.string(forKey: activeGroupKey) else { return nil }
        return UUID(uuidString: idString)
    }

    func saveActiveGroupID(_ id: UUID?) {
        defaults.set(id?.uuidString, forKey: activeGroupKey)
    }
}
