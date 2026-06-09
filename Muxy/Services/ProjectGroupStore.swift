import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "ProjectGroupStore")

@MainActor
@Observable
final class ProjectGroupStore {
    private(set) var groups: [ProjectGroup] = []
    private(set) var activeGroupID: UUID?
    private let persistence: any ProjectGroupPersisting

    init(persistence: any ProjectGroupPersisting) {
        self.persistence = persistence
        load()
    }

    func selectGroup(id: UUID) {
        activeGroupID = id
        persistence.saveActiveGroupID(id)
    }

    func clearGroupSelection() {
        activeGroupID = nil
        persistence.saveActiveGroupID(nil)
    }

    func filteredProjects(from projects: [Project]) -> [Project] {
        guard let activeGroupID else { return projects }
        guard let group = groups.first(where: { $0.id == activeGroupID }) else { return projects }
        return projects.filter { group.projectIDs.contains($0.id) }
    }

    func addGroup(name: String) {
        let sortOrder = groups.count
        let group = ProjectGroup(name: name, sortOrder: sortOrder)
        groups.append(group)
        save()
    }

    func removeGroup(id: UUID) {
        if activeGroupID == id {
            activeGroupID = nil
            persistence.saveActiveGroupID(nil)
        }
        groups.removeAll { $0.id == id }
        save()
    }

    func renameGroup(id: UUID, to newName: String) {
        guard let index = groups.firstIndex(where: { $0.id == id }) else { return }
        groups[index].name = newName
        save()
    }

    func addProject(projectID: UUID, toGroup groupID: UUID) {
        guard projectID != Project.homeID else { return }
        guard let index = groups.firstIndex(where: { $0.id == groupID }) else { return }
        for otherIndex in groups.indices where otherIndex != index {
            groups[otherIndex].projectIDs.removeAll { $0 == projectID }
        }
        if !groups[index].projectIDs.contains(projectID) {
            groups[index].projectIDs.append(projectID)
        }
        save()
    }

    func addProjectToActiveGroup(projectID: UUID) {
        guard let activeGroupID else { return }
        addProject(projectID: projectID, toGroup: activeGroupID)
    }

    func removeProject(projectID: UUID, fromGroup groupID: UUID) {
        guard let index = groups.firstIndex(where: { $0.id == groupID }) else { return }
        groups[index].projectIDs.removeAll { $0 == projectID }
        save()
    }

    func removeProjectFromAllGroups(projectID: UUID) {
        for index in groups.indices {
            groups[index].projectIDs.removeAll { $0 == projectID }
        }
        save()
    }

    private func save() {
        do {
            try persistence.saveProjectGroups(groups)
        } catch {
            logger.error("Failed to save project groups: \(error)")
        }
    }

    private func load() {
        do {
            let loaded = try persistence.loadProjectGroups()
            groups = loaded.sorted { $0.sortOrder < $1.sortOrder }
        } catch {
            logger.error("Failed to load project groups: \(error)")
        }
        let storedActive = persistence.loadActiveGroupID()
        if let storedActive, groups.contains(where: { $0.id == storedActive }) {
            activeGroupID = storedActive
        } else if storedActive != nil {
            persistence.saveActiveGroupID(nil)
        }
    }
}
