import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "ProjectStore")

@MainActor
@Observable
final class ProjectStore {
    private(set) var storedProjects: [Project] = []
    private let persistence: any ProjectPersisting
    var onProjectRemoved: ((UUID) -> Void)?

    init(persistence: any ProjectPersisting) {
        self.persistence = persistence
        load()
    }

    var projects: [Project] {
        [Project.home] + storedProjects
    }

    func add(_ project: Project) {
        storedProjects.append(project)
        save()
    }

    func remove(id: UUID) {
        guard id != Project.homeID else { return }
        storedProjects.removeAll { $0.id == id }
        save()
        onProjectRemoved?(id)
    }

    func rename(id: UUID, to newName: String) {
        guard let index = storedProjects.firstIndex(where: { $0.id == id }) else { return }
        storedProjects[index].name = newName
        save()
    }

    func setLogo(id: UUID, to logo: String?) {
        guard let index = storedProjects.firstIndex(where: { $0.id == id }) else { return }
        if logo == nil {
            ProjectLogoStorage.remove(forProjectID: id)
        }
        storedProjects[index].logo = logo
        save()
    }

    func setIcon(id: UUID, to icon: String?) {
        guard let index = storedProjects.firstIndex(where: { $0.id == id }) else { return }
        storedProjects[index].icon = icon
        save()
    }

    func setIconColor(id: UUID, to color: String?) {
        guard let index = storedProjects.firstIndex(where: { $0.id == id }) else { return }
        storedProjects[index].iconColor = color
        save()
    }

    func setPreferredWorktreeParentPath(id: UUID, to path: String?) {
        guard let index = storedProjects.firstIndex(where: { $0.id == id }) else { return }
        storedProjects[index].preferredWorktreeParentPath = WorktreeLocationResolver.normalizedPath(path)
        save()
    }

    func reorder(fromOffsets source: IndexSet, toOffset destination: Int) {
        storedProjects.move(fromOffsets: source, toOffset: destination)
        for index in storedProjects.indices {
            storedProjects[index].sortOrder = index
        }
        save()
    }

    func save() {
        do {
            try persistence.saveProjects(storedProjects)
        } catch {
            logger.error("Failed to save projects: \(error)")
        }
    }

    private func load() {
        do {
            storedProjects = try persistence.loadProjects().filter { !$0.isHome }
            storedProjects.sort { $0.sortOrder < $1.sortOrder }
        } catch {
            logger.error("Failed to load projects: \(error)")
        }
    }
}
