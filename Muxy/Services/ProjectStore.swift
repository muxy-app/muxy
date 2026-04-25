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
        if let project = projects.first(where: { $0.id == id }) {
            VCSPersistedSettings.clearSettings(repoPath: project.path)
        }
        projects.removeAll { $0.id == id }
        save()
    }

    func rename(id: UUID, to newName: String) {
        guard let index = projects.firstIndex(where: { $0.id == id }) else { return }
        projects[index].name = newName
        save()
    }

    func setLogo(id: UUID, to logo: String?) {
        guard let index = projects.firstIndex(where: { $0.id == id }) else { return }
        if logo == nil {
            ProjectLogoStorage.remove(forProjectID: id)
        }
        projects[index].logo = logo
        save()
    }

    func setIconColor(id: UUID, to color: String?) {
        guard let index = projects.firstIndex(where: { $0.id == id }) else { return }
        projects[index].iconColor = color
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
