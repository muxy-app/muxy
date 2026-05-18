import Foundation

protocol ProjectGroupPersisting {
    func loadGroups() throws -> [ProjectGroup]
    func saveGroups(_ groups: [ProjectGroup]) throws
}

final class FileProjectGroupPersistence: ProjectGroupPersisting {
    private let store: CodableFileStore<[ProjectGroup]>

    init(fileURL: URL = MuxyFileStorage.fileURL(filename: "project-groups.json")) {
        store = CodableFileStore(fileURL: fileURL)
    }

    func loadGroups() throws -> [ProjectGroup] {
        try store.load() ?? []
    }

    func saveGroups(_ groups: [ProjectGroup]) throws {
        try store.save(groups)
    }
}
