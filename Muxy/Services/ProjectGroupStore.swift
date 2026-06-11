import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "ProjectGroupStore")

@MainActor
@Observable
final class ProjectGroupStore {
    private(set) var groups: [ProjectGroup] = []
    private(set) var activeGroupID: UUID?
    private let persistence: any ProjectGroupPersisting
    private let workspaceContextSink: any WorkspaceContextSink

    init(
        persistence: any ProjectGroupPersisting,
        workspaceContextSink: any WorkspaceContextSink = ActiveWorkspaceContext.shared
    ) {
        self.persistence = persistence
        self.workspaceContextSink = workspaceContextSink
        load()
    }

    func selectGroup(id: UUID) {
        activeGroupID = id
        persistence.saveActiveGroupID(id)
        syncActiveWorkspaceContext()
    }

    func clearGroupSelection() {
        activeGroupID = nil
        persistence.saveActiveGroupID(nil)
        syncActiveWorkspaceContext()
    }

    private func syncActiveWorkspaceContext() {
        workspaceContextSink.update(activeWorkspaceContext)
    }

    func filteredProjects(from projects: [Project]) -> [Project] {
        guard let group = activeGroup else { return projects }
        guard group.type == .local else { return [] }
        return projects.filter { group.projectIDs.contains($0.id) }
    }

    var activeRemoteProjects: [RemoteProject] {
        guard let group = activeGroup, group.type == .ssh else { return [] }
        return group.remoteProjects
    }

    var activeRemoteHomeProject: Project? {
        activeGroup?.remoteHomeProject
    }

    var activeRemoteProjectIDs: Set<UUID> {
        guard isRemoteWorkspaceActive else { return [] }
        var ids = Set(activeRemoteProjects.map(\.id))
        if let homeID = activeRemoteHomeProject?.id { ids.insert(homeID) }
        return ids
    }

    var isRemoteWorkspaceActive: Bool {
        activeGroup?.type == .ssh
    }

    func displayProjects(localProjects: [Project]) -> [Project] {
        guard let group = activeGroup, group.type == .ssh else {
            return filteredProjects(from: localProjects)
        }
        return group.remoteProjects.enumerated().map { index, remote in
            remote.asProject(workspaceID: group.id, sortOrder: index)
        }
    }

    var activeGroup: ProjectGroup? {
        guard let activeGroupID else { return nil }
        return groups.first(where: { $0.id == activeGroupID })
    }

    var activeWorkspaceContext: WorkspaceContext {
        activeGroup?.workspaceContext ?? .local
    }

    func workspaceContext(for project: Project) -> WorkspaceContext {
        guard let workspaceID = project.remoteWorkspaceID else { return .local }
        return groups.first(where: { $0.id == workspaceID })?.workspaceContext ?? .local
    }

    func addGroup(name: String) {
        let sortOrder = groups.count
        let group = ProjectGroup(name: name, sortOrder: sortOrder)
        groups.append(group)
        save()
    }

    @discardableResult
    func addSSHWorkspace(name: String, data: SSHWorkspaceData) -> ProjectGroup {
        let group = ProjectGroup(
            name: name,
            sortOrder: groups.count,
            type: .ssh,
            sshData: data
        )
        groups.append(group)
        save()
        return group
    }

    func updateSSHWorkspace(id: UUID, data: SSHWorkspaceData) {
        guard let index = groups.firstIndex(where: { $0.id == id }) else { return }
        groups[index].sshData = data
        save()
    }

    @discardableResult
    func addRemoteProject(name: String, path: String, toGroup groupID: UUID) -> RemoteProject? {
        guard let index = groups.firstIndex(where: { $0.id == groupID }) else { return nil }
        let standardizedPath = ProjectPickerPathService.standardizedRemotePath(path)
        if let remoteRoot = groups[index].sshData?.remoteRoot,
           standardizedPath == ProjectPickerPathService.standardizedRemotePath(remoteRoot)
        {
            return nil
        }
        if let existing = groups[index].remoteProjects.first(where: {
            ProjectPickerPathService.standardizedRemotePath($0.path) == standardizedPath
        }) {
            return existing
        }
        let project = RemoteProject(name: name, path: path)
        groups[index].remoteProjects.append(project)
        save()
        return project
    }

    func removeRemoteProject(id: UUID, fromGroup groupID: UUID) {
        guard let index = groups.firstIndex(where: { $0.id == groupID }) else { return }
        groups[index].remoteProjects.removeAll { $0.id == id }
        save()
    }

    func updateRemoteProject(id: UUID, _ mutate: (inout RemoteProject) -> Void) {
        for groupIndex in groups.indices {
            guard let projectIndex = groups[groupIndex].remoteProjects.firstIndex(where: { $0.id == id })
            else { continue }
            mutate(&groups[groupIndex].remoteProjects[projectIndex])
            save()
            return
        }
    }

    func renameRemoteProject(id: UUID, to name: String) {
        updateRemoteProject(id: id) { $0.name = name }
    }

    func setRemoteProjectWorktreesEnabled(id: UUID, to enabled: Bool) {
        updateRemoteProject(id: id) { $0.worktreesEnabled = enabled }
    }

    func removeGroup(id: UUID) {
        if activeGroupID == id {
            activeGroupID = nil
            persistence.saveActiveGroupID(nil)
            syncActiveWorkspaceContext()
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
        syncActiveWorkspaceContext()
    }
}
