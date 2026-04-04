import Foundation

protocol ProjectPersisting {
    func loadProjects() throws -> [Project]
    func saveProjects(_ projects: [Project]) throws
}

final class FileProjectPersistence: ProjectPersisting {
    private let fileURL: URL

    init(fileURL: URL = MuxyFileStorage.fileURL(filename: "projects.json")) {
        self.fileURL = fileURL
    }

    func loadProjects() throws -> [Project] {
        guard FileManager.default.fileExists(atPath: fileURL.path) else { return [] }
        let data = try Data(contentsOf: fileURL)
        return try JSONDecoder().decode([Project].self, from: data)
    }

    func saveProjects(_ projects: [Project]) throws {
        let data = try JSONEncoder().encode(projects)
        try data.write(to: fileURL, options: .atomic)
    }
}
