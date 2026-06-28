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
    private let remoteDeviceStore: RemoteDeviceStore

    init(
        persistence: any ProjectGroupPersisting,
        remoteDeviceStore: RemoteDeviceStore,
        workspaceContextSink: any WorkspaceContextSink = ActiveWorkspaceContext.shared
    ) {
        self.persistence = persistence
        self.remoteDeviceStore = remoteDeviceStore
        self.workspaceContextSink = workspaceContextSink
        load()
    }

    private func device(for group: ProjectGroup) -> RemoteDevice? {
        remoteDeviceStore.device(id: group.remoteDeviceID)
    }

    func selectGroup(id: UUID) {
        activeGroupID = id
        persistence.saveActiveGroupID(id)
        syncActiveWorkspaceContext()
    }

    func groupID(containing project: Project) -> UUID? {
        if let workspaceID = project.remoteWorkspaceID {
            return workspaceID
        }
        return groups.first { $0.projectIDs.contains(project.id) }?.id
    }

    func activateWorkspace(containing project: Project) {
        let groupID = groupID(containing: project)
        guard groupID != activeGroupID else { return }
        guard let groupID else {
            clearGroupSelection()
            return
        }
        selectGroup(id: groupID)
    }

    func activateWorkspaceForProjectSelection(containing project: Project) {
        guard activeGroupID != nil else { return }
        activateWorkspace(containing: project)
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
        guard let group = activeGroup else { return nil }
        return group.remoteHomeProject(device: device(for: group))
    }

    func remoteHomeProject(for group: ProjectGroup) -> Project? {
        group.remoteHomeProject(device: device(for: group))
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

    func displayProjects(localProjects: [Project], sortMode: ProjectSortMode = .current) -> [Project] {
        guard let group = activeGroup, group.type == .ssh else {
            return sortMode.sorted(filteredProjects(from: localProjects))
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
        guard let group = activeGroup else { return .local }
        return group.workspaceContext(device: device(for: group))
    }

    var remoteProjects: [Project] {
        groups.flatMap { group -> [Project] in
            guard group.type == .ssh else { return [] }
            let device = device(for: group)
            let home = group.remoteHomeProject(device: device).map { [$0] } ?? []
            let projects = group.remoteProjects.enumerated().map { index, remote in
                remote.asProject(workspaceID: group.id, sortOrder: index)
            }
            return home + projects
        }
    }

    func resolveProject(identifier: String?, localProjects: [Project], activeProjectID: UUID?) -> Project? {
        let candidates = localProjects + remoteProjects
        if let identifier, !identifier.isEmpty {
            return Self.matchProject(identifier, in: candidates)
        }
        guard let activeProjectID else { return nil }
        return candidates.first { $0.id == activeProjectID }
    }

    static func matchProject(_ identifier: String, in projects: [Project]) -> Project? {
        let standardizedPath = ProjectPickerPathService.standardizedRemotePath(identifier)
        return projects.first { project in
            project.id.uuidString == identifier
                || project.name.localizedCaseInsensitiveCompare(identifier) == .orderedSame
                || ProjectPickerPathService.standardizedRemotePath(project.path) == standardizedPath
        }
    }

    func workspaceContext(for project: Project) -> WorkspaceContext {
        if let deviceID = project.remoteDeviceID,
           let device = remoteDeviceStore.device(id: deviceID)
        {
            return .ssh(device.destination)
        }
        guard let workspaceID = project.remoteWorkspaceID,
              let group = groups.first(where: { $0.id == workspaceID })
        else { return .local }
        return group.workspaceContext(device: device(for: group))
    }

    func device(for project: Project) -> RemoteDevice? {
        if let deviceID = project.remoteDeviceID {
            return remoteDeviceStore.device(id: deviceID)
        }
        guard let workspaceID = project.remoteWorkspaceID,
              let group = groups.first(where: { $0.id == workspaceID })
        else { return nil }
        return device(for: group)
    }

    func addGroup(name: String) {
        let sortOrder = groups.count
        let group = ProjectGroup(name: name, sortOrder: sortOrder)
        groups.append(group)
        save()
    }

    @discardableResult
    func addRemoteWorkspace(name: String, deviceID: UUID) -> ProjectGroup {
        let group = ProjectGroup(
            name: name,
            sortOrder: groups.count,
            type: .ssh,
            remoteDeviceID: deviceID
        )
        groups.append(group)
        save()
        return group
    }

    func updateRemoteWorkspace(id: UUID, deviceID: UUID) {
        guard let index = groups.firstIndex(where: { $0.id == id }) else { return }
        groups[index].remoteDeviceID = deviceID
        save()
    }

    func workspaceNames(usingDevice deviceID: UUID) -> [String] {
        groups.filter { $0.remoteDeviceID == deviceID }.map(\.name)
    }

    func removeWorkspaces(usingDevice deviceID: UUID) {
        let affected = groups.filter { $0.remoteDeviceID == deviceID }
        for group in affected {
            removeGroup(id: group.id)
        }
    }

    @discardableResult
    func addRemoteProject(name: String, path: String, toGroup groupID: UUID) -> RemoteProject? {
        guard let index = groups.firstIndex(where: { $0.id == groupID }) else { return nil }
        let standardizedPath = ProjectPickerPathService.standardizedRemotePath(path)
        if let remoteRoot = device(for: groups[index])?.ssh.remoteRoot,
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
        migrateLegacySSHWorkspaces()
        let storedActive = persistence.loadActiveGroupID()
        if let storedActive, groups.contains(where: { $0.id == storedActive }) {
            activeGroupID = storedActive
        } else if storedActive != nil {
            persistence.saveActiveGroupID(nil)
        }
        syncActiveWorkspaceContext()
    }

    private func migrateLegacySSHWorkspaces() {
        var didMigrate = false
        for index in groups.indices {
            guard groups[index].type == .ssh,
                  groups[index].remoteDeviceID == nil,
                  let legacy = groups[index].legacySSHData
            else { continue }
            let device = remoteDeviceStore.add(name: groups[index].name, ssh: legacy)
            groups[index].remoteDeviceID = device.id
            groups[index].legacySSHData = nil
            didMigrate = true
        }
        guard didMigrate else { return }
        save()
    }
}
