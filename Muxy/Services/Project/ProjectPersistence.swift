import Foundation

protocol ProjectPersisting {
    func loadProjects() throws -> [Project]
    func saveProjects(_ projects: [Project]) throws
    func loadRecentlyRemovedProjects() throws -> [RecentlyRemovedProject]
    func saveRecentlyRemovedProjects(_ projects: [RecentlyRemovedProject]) throws
}

extension ProjectPersisting {
    func loadRecentlyRemovedProjects() throws -> [RecentlyRemovedProject] {
        []
    }

    func saveRecentlyRemovedProjects(_: [RecentlyRemovedProject]) throws {}
}

final class FileProjectPersistence: ProjectPersisting {
    private let projectsStore: CodableFileStore<[Project]>
    private let recentlyRemovedProjectsStore: CodableFileStore<[RecentlyRemovedProject]>

    init(
        fileURL: URL = MuxyFileStorage.fileURL(filename: "projects.json"),
        recentlyRemovedFileURL: URL = MuxyFileStorage.fileURL(filename: "recently-removed-projects.json")
    ) {
        projectsStore = CodableFileStore(fileURL: fileURL)
        recentlyRemovedProjectsStore = CodableFileStore(fileURL: recentlyRemovedFileURL)
    }

    func loadProjects() throws -> [Project] {
        try projectsStore.load() ?? []
    }

    func saveProjects(_ projects: [Project]) throws {
        try projectsStore.save(projects)
    }

    func loadRecentlyRemovedProjects() throws -> [RecentlyRemovedProject] {
        try recentlyRemovedProjectsStore.load() ?? []
    }

    func saveRecentlyRemovedProjects(_ projects: [RecentlyRemovedProject]) throws {
        try recentlyRemovedProjectsStore.save(projects)
    }
}
