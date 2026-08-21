import AppKit

@MainActor
struct ProjectEditingService {
    let projectStore: ProjectStore
    let projectGroupStore: ProjectGroupStore

    @discardableResult
    func rename(_ project: Project, to name: String) -> Bool {
        update(project, to: name, remoteKeyPath: \.name) { store, id, value in
            store.rename(id: id, to: value)
        }
    }

    @discardableResult
    func setLogo(_ project: Project, to logo: String?) -> Bool {
        guard project.remoteWorkspaceID != nil else {
            return projectStore.setLogo(id: project.id, to: logo)
        }
        let didUpdate = projectGroupStore.updateRemoteProject(id: project.id) { remoteProject in
            remoteProject.logo = logo
        }
        if didUpdate, logo == nil {
            ProjectLogoStorage.remove(forProjectID: project.id)
        }
        return didUpdate
    }

    @discardableResult
    func setLogo(_ project: Project, croppedImage: NSImage) -> Bool {
        guard let storedProject = storedProject(for: project) else { return false }
        guard let logo = ProjectLogoStorage.save(croppedImage: croppedImage, forProjectID: project.id) else { return false }
        guard storedProject.logo != logo else { return true }
        guard setLogo(project, to: logo) else {
            ProjectLogoStorage.remove(forProjectID: project.id)
            return false
        }
        return true
    }

    @discardableResult
    func setIcon(_ project: Project, to icon: String?) -> Bool {
        update(project, to: icon, remoteKeyPath: \.icon) { store, id, value in
            store.setIcon(id: id, to: value)
        }
    }

    @discardableResult
    func setIconColor(_ project: Project, to color: String?) -> Bool {
        update(project, to: color, remoteKeyPath: \.iconColor) { store, id, value in
            store.setIconColor(id: id, to: value)
        }
    }

    @discardableResult
    func setWorktreesEnabled(_ project: Project, to enabled: Bool) -> Bool {
        update(project, to: enabled, remoteKeyPath: \.worktreesEnabled) { store, id, value in
            store.setWorktreesEnabled(id: id, to: value)
        }
    }

    @discardableResult
    func setPinned(_ project: Project, to pinned: Bool) -> Bool {
        update(project, to: pinned, remoteKeyPath: \.isPinned) { store, id, value in
            store.setPinned(id: id, to: value)
        }
    }

    private func update<Value>(
        _ project: Project,
        to value: Value,
        remoteKeyPath: WritableKeyPath<RemoteProject, Value>,
        localUpdate: (ProjectStore, UUID, Value) -> Bool
    ) -> Bool {
        guard project.remoteWorkspaceID != nil else {
            return localUpdate(projectStore, project.id, value)
        }
        return projectGroupStore.updateRemoteProject(id: project.id) { remoteProject in
            remoteProject[keyPath: remoteKeyPath] = value
        }
    }

    private func storedProject(for project: Project) -> Project? {
        guard let workspaceID = project.remoteWorkspaceID else {
            return projectStore.storedProjects.first { $0.id == project.id }
        }
        guard let group = projectGroupStore.groups.first(where: { $0.id == workspaceID }),
              let index = group.remoteProjects.firstIndex(where: { $0.id == project.id })
        else { return nil }
        return group.remoteProjects[index].asProject(workspaceID: workspaceID, sortOrder: index)
    }
}
