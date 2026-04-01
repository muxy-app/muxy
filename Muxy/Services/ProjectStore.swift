import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "ProjectStore")

@MainActor
@Observable
final class ProjectStore {
    private(set) var projects: [Project] = []
    private let persistence: any ProjectPersisting

    init(persistence: any ProjectPersisting) {
        self.persistence = persistence
        load()
    }

    func add(_ project: Project) {
        projects.append(project)
        save()
    }

    func remove(id: UUID) {
        projects.removeAll { $0.id == id }
        save()
    }

    func reorder(fromOffsets source: IndexSet, toOffset destination: Int) {
        projects.move(fromOffsets: source, toOffset: destination)
        for index in projects.indices {
            projects[index].sortOrder = index
        }
        save()
    }

    func save() {
        do {
            try persistence.saveProjects(projects)
        } catch {
            logger.error("Failed to save projects: \(error)")
        }
    }

    private func load() {
        do {
            projects = try persistence.loadProjects()
            projects.sort { $0.sortOrder < $1.sortOrder }
        } catch {
            logger.error("Failed to load projects: \(error)")
        }
    }
}
