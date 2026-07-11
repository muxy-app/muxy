import Foundation
import MuxyShared
import os

private let logger = Logger(subsystem: "app.muxy", category: "ProjectStore")

@MainActor
@Observable
final class ProjectStore {
    private(set) var storedProjects: [Project] = []
    private let persistence: any ProjectPersisting
    var onProjectRemoved: ((UUID) -> Void)?
    var onProjectsChanged: (() -> Void)?

    init(persistence: any ProjectPersisting) {
        self.persistence = persistence
        load()
    }

    var projects: [Project] {
        [Project.home] + storedProjects
    }

    private var usedIconColors: Set<String> {
        Set(storedProjects.compactMap(\.iconColor))
    }

    func add(_ project: Project) {
        var project = project
        if project.iconColor == nil {
            project.iconColor = ProjectIconColor.randomSwatch(excluding: usedIconColors)?.id
        }
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

    func setPinned(id: UUID, to pinned: Bool) {
        guard let index = storedProjects.firstIndex(where: { $0.id == id }) else { return }
        storedProjects[index].isPinned = pinned
        save()
    }

    func setWorktreesEnabled(id: UUID, to enabled: Bool) {
        guard let index = storedProjects.firstIndex(where: { $0.id == id }) else { return }
        storedProjects[index].worktreesEnabled = enabled
        save()
    }

    func setPreferredWorktreeParentPath(id: UUID, to path: String?) {
        guard let index = storedProjects.firstIndex(where: { $0.id == id }) else { return }
        storedProjects[index].preferredWorktreeParentPath = WorktreeLocationResolver.normalizedPath(path)
        save()
    }

    func markActive(id: UUID) {
        guard let index = storedProjects.firstIndex(where: { $0.id == id }) else { return }
        storedProjects[index].lastActiveAt = Date()
        save(notify: false)
    }

    func persistOrder(_ orderedIDs: [UUID]) {
        let positions = Dictionary(uniqueKeysWithValues: orderedIDs.enumerated().map { ($1, $0) })
        storedProjects.sort { positions[$0.id, default: Int.max] < positions[$1.id, default: Int.max] }
        for index in storedProjects.indices {
            storedProjects[index].sortOrder = index
        }
        save()
    }

    func persistOrder(_ orderedIDs: [UUID], scopedTo scopedIDs: Set<UUID>) {
        let projectsByID = Dictionary(uniqueKeysWithValues: storedProjects.map { ($0.id, $0) })
        var scopedProjects = orderedIDs.compactMap { projectsByID[$0] }
        storedProjects = storedProjects.map { project in
            guard scopedIDs.contains(project.id), !scopedProjects.isEmpty else { return project }
            return scopedProjects.removeFirst()
        }
        for index in storedProjects.indices {
            storedProjects[index].sortOrder = index
        }
        save()
    }

    func reorder(fromOffsets source: IndexSet, toOffset destination: Int) {
        storedProjects.move(fromOffsets: source, toOffset: destination)
        for index in storedProjects.indices {
            storedProjects[index].sortOrder = index
        }
        save()
    }

    func save(notify: Bool = true) {
        do {
            try persistence.saveProjects(storedProjects)
        } catch {
            logger.error("Failed to save projects: \(error)")
        }
        if notify { onProjectsChanged?() }
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
