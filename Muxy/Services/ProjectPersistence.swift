import Foundation

protocol ProjectPersisting {
    func loadProjects() throws -> [Project]
    func saveProjects(_ projects: [Project]) throws
}

final class FileProjectPersistence: ProjectPersisting {
    private let fileURL: URL

    init(fileURL: URL = FileProjectPersistence.defaultFileURL()) {
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

    private static func defaultFileURL() -> URL {
        guard let appSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first else {
            fatalError("Application Support directory unavailable")
        }
        let dir = appSupport.appendingPathComponent("Muxy", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        return dir.appendingPathComponent("projects.json")
    }
}
