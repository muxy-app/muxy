import Foundation

protocol ProjectPersisting {
    func loadProjects() throws -> [Project]
    func saveProjects(_ projects: [Project]) throws
}

final class FileProjectPersistence: ProjectPersisting {
    private let store: CodableFileStore<[Project]>

    init(fileURL: URL = MuxyFileStorage.fileURL(filename: "projects.json")) {
        store = CodableFileStore(fileURL: fileURL)
    }

    func loadProjects() throws -> [Project] {
        try store.load() ?? []
    }

    func saveProjects(_ projects: [Project]) throws {
        try store.save(projects)
    }
}
