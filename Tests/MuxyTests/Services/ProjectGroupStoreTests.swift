import Foundation
import Testing

@testable import Muxy

@Suite("ProjectGroupStore")
@MainActor
struct ProjectGroupStoreTests {
    @Test("addGroup appends a new group and persists it")
    func addGroup() {
        let persistence = ProjectGroupPersistenceStub()
        let store = ProjectGroupStore(persistence: persistence)

        store.addGroup(name: "Work")

        #expect(store.groups.count == 1)
        #expect(store.groups.first?.name == "Work")
        #expect(persistence.savedGroups?.count == 1)
    }

    @Test("removeGroup deletes the group and persists")
    func removeGroup() {
        let group = ProjectGroup(name: "Work")
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)

        store.removeGroup(id: group.id)

        #expect(store.groups.isEmpty)
        #expect(persistence.savedGroups?.isEmpty == true)
    }

    @Test("removeGroup clears activeGroupID when active group is deleted")
    func removeGroupClearsActiveGroup() {
        let group = ProjectGroup(name: "Work")
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)
        store.selectGroup(id: group.id)

        store.removeGroup(id: group.id)

        #expect(store.activeGroupID == nil)
        #expect(persistence.storedActiveGroupID == nil)
    }

    @Test("renameGroup updates the name and persists")
    func renameGroup() {
        let group = ProjectGroup(name: "Work")
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)

        store.renameGroup(id: group.id, to: "Personal")

        #expect(store.groups.first?.name == "Personal")
        #expect(persistence.savedGroups?.first?.name == "Personal")
    }

    @Test("renameGroup with unknown id is a no-op")
    func renameGroupUnknownID() {
        let group = ProjectGroup(name: "Work")
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)

        store.renameGroup(id: UUID(), to: "Other")

        #expect(store.groups.first?.name == "Work")
    }

    @Test("addProject adds projectID to the group and persists")
    func addProject() {
        let group = ProjectGroup(name: "Work")
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)
        let projectID = UUID()

        store.addProject(projectID: projectID, toGroup: group.id)

        #expect(store.groups.first?.projectIDs == [projectID])
        #expect(persistence.savedGroups?.first?.projectIDs == [projectID])
    }

    @Test("addProject never adds the Home project to a group")
    func addProjectIgnoresHome() {
        let group = ProjectGroup(name: "Work")
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)

        store.addProject(projectID: Project.homeID, toGroup: group.id)

        #expect(store.groups.first?.projectIDs.isEmpty == true)
    }

    @Test("addProject ignores duplicate projectID")
    func addProjectDuplicate() {
        let projectID = UUID()
        let group = ProjectGroup(name: "Work", projectIDs: [projectID])
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)

        store.addProject(projectID: projectID, toGroup: group.id)

        #expect(store.groups.first?.projectIDs.count == 1)
    }

    @Test("addProjectToActiveGroup adds project to selected group and persists")
    func addProjectToActiveGroup() {
        let group = ProjectGroup(name: "Work")
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)
        let projectID = UUID()

        store.selectGroup(id: group.id)
        store.addProjectToActiveGroup(projectID: projectID)

        #expect(store.groups.first?.projectIDs == [projectID])
        #expect(persistence.savedGroups?.first?.projectIDs == [projectID])
    }

    @Test("addProjectToActiveGroup preserves All Projects behavior")
    func addProjectToActiveGroupNoSelection() {
        let group = ProjectGroup(name: "Work")
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)

        store.addProjectToActiveGroup(projectID: UUID())

        #expect(store.groups.first?.projectIDs.isEmpty == true)
        #expect(persistence.savedGroups == nil)
    }

    @Test("removeProject removes projectID from the group and persists")
    func removeProject() {
        let projectID = UUID()
        let group = ProjectGroup(name: "Work", projectIDs: [projectID])
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)

        store.removeProject(projectID: projectID, fromGroup: group.id)

        #expect(store.groups.first?.projectIDs.isEmpty == true)
        #expect(persistence.savedGroups?.first?.projectIDs.isEmpty == true)
    }

    @Test("load on empty persistence yields empty groups")
    func loadEmptyIsEmpty() {
        let persistence = ProjectGroupPersistenceStub(initial: [])
        let store = ProjectGroupStore(persistence: persistence)

        #expect(store.groups.isEmpty)
    }

    @Test("load sorts groups by sortOrder")
    func loadSortsByOrder() {
        let second = ProjectGroup(name: "B", sortOrder: 1)
        let first = ProjectGroup(name: "A", sortOrder: 0)
        let persistence = ProjectGroupPersistenceStub(initial: [second, first])
        let store = ProjectGroupStore(persistence: persistence)

        #expect(store.groups.first?.name == "A")
        #expect(store.groups.last?.name == "B")
    }

    @Test("addGroup assigns sequential sortOrder")
    func addGroupSortOrder() {
        let persistence = ProjectGroupPersistenceStub(initial: [])
        let store = ProjectGroupStore(persistence: persistence)

        store.addGroup(name: "First")
        store.addGroup(name: "Second")

        #expect(store.groups[0].sortOrder == 0)
        #expect(store.groups[1].sortOrder == 1)
    }

    @Test("activeGroupID is nil by default")
    func activeGroupIDDefaultsToNil() {
        let persistence = ProjectGroupPersistenceStub(initial: [])
        let store = ProjectGroupStore(persistence: persistence)

        #expect(store.activeGroupID == nil)
    }

    @Test("selectGroup sets activeGroupID and persists it")
    func selectGroup() {
        let group = ProjectGroup(name: "Work")
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)

        store.selectGroup(id: group.id)

        #expect(store.activeGroupID == group.id)
        #expect(persistence.storedActiveGroupID == group.id)
    }

    @Test("clearGroupSelection resets activeGroupID to nil and persists")
    func clearGroupSelection() {
        let group = ProjectGroup(name: "Work")
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)
        store.selectGroup(id: group.id)

        store.clearGroupSelection()

        #expect(store.activeGroupID == nil)
        #expect(persistence.storedActiveGroupID == nil)
    }

    @Test("load restores persisted activeGroupID when group still exists")
    func loadRestoresActiveGroupID() {
        let group = ProjectGroup(name: "Work")
        let persistence = ProjectGroupPersistenceStub(initial: [group], storedActiveGroupID: group.id)
        let store = ProjectGroupStore(persistence: persistence)

        #expect(store.activeGroupID == group.id)
    }

    @Test("load discards persisted activeGroupID when group no longer exists")
    func loadDiscardsOrphanActiveGroupID() {
        let persistence = ProjectGroupPersistenceStub(initial: [], storedActiveGroupID: UUID())
        let store = ProjectGroupStore(persistence: persistence)

        #expect(store.activeGroupID == nil)
        #expect(persistence.storedActiveGroupID == nil)
    }

    @Test("filteredProjects returns all projects when activeGroupID is nil")
    func filteredProjectsAllWhenNoSelection() {
        let persistence = ProjectGroupPersistenceStub(initial: [])
        let store = ProjectGroupStore(persistence: persistence)
        let projects = [
            Project(name: "A", path: "/a"),
            Project(name: "B", path: "/b")
        ]

        let result = store.filteredProjects(from: projects)

        #expect(result.count == 2)
    }

    @Test("filteredProjects returns only group projects when a group is selected")
    func filteredProjectsActiveGroup() {
        let projectA = Project(name: "A", path: "/a")
        let projectB = Project(name: "B", path: "/b")
        let group = ProjectGroup(name: "Work", projectIDs: [projectA.id])
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)
        store.selectGroup(id: group.id)

        let result = store.filteredProjects(from: [projectA, projectB])

        #expect(result.count == 1)
        #expect(result.first?.id == projectA.id)
    }

    @Test("filteredProjects returns all projects when activeGroupID does not match any group")
    func filteredProjectsUnknownActiveGroup() {
        let persistence = ProjectGroupPersistenceStub(initial: [])
        let store = ProjectGroupStore(persistence: persistence)
        store.selectGroup(id: UUID())
        let projects = [Project(name: "A", path: "/a")]

        let result = store.filteredProjects(from: projects)

        #expect(result.count == 1)
    }

    @Test("filteredProjects returns empty array when group has no matching projects")
    func filteredProjectsEmptyGroup() {
        let group = ProjectGroup(name: "Empty")
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)
        store.selectGroup(id: group.id)
        let projects = [Project(name: "A", path: "/a")]

        let result = store.filteredProjects(from: projects)

        #expect(result.isEmpty)
    }

    @Test("workspaceContext is local for a project without a remote workspace")
    func workspaceContextLocalProject() {
        let store = ProjectGroupStore(persistence: ProjectGroupPersistenceStub(initial: []))

        #expect(store.workspaceContext(for: Project(name: "A", path: "/a")) == .local)
    }

    @Test("workspaceContext resolves a remote project to its group's SSH context")
    func workspaceContextRemoteProject() {
        let sshData = SSHWorkspaceData(host: "example.com", remoteRoot: "~/code", user: "deploy")
        let group = ProjectGroup(name: "Remote", type: .ssh, sshData: sshData)
        let store = ProjectGroupStore(persistence: ProjectGroupPersistenceStub(initial: [group]))
        let project = Project(name: "api", path: "~/code/api", remoteWorkspaceID: group.id)

        #expect(store.workspaceContext(for: project) == .ssh(sshData.destination))
    }

    @Test("workspaceContext falls back to local when the remote workspace is missing")
    func workspaceContextOrphanRemoteProject() {
        let store = ProjectGroupStore(persistence: ProjectGroupPersistenceStub(initial: []))
        let project = Project(name: "api", path: "~/code/api", remoteWorkspaceID: UUID())

        #expect(store.workspaceContext(for: project) == .local)
    }

    @Test("addSSHWorkspace appends an SSH group with its data and persists")
    func addSSHWorkspace() {
        let persistence = ProjectGroupPersistenceStub()
        let store = ProjectGroupStore(persistence: persistence)
        let data = SSHWorkspaceData(host: "example.com", remoteRoot: "~/code", user: "deploy")

        let group = store.addSSHWorkspace(name: "Remote", data: data)

        #expect(store.groups.count == 1)
        #expect(store.groups.first?.type == .ssh)
        #expect(store.groups.first?.sshData == data)
        #expect(persistence.savedGroups?.first?.id == group.id)
    }

    @Test("updateSSHWorkspace replaces the SSH data and persists")
    func updateSSHWorkspace() {
        let group = ProjectGroup(name: "Remote", type: .ssh, sshData: SSHWorkspaceData(host: "old.example.com"))
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)
        let updated = SSHWorkspaceData(host: "new.example.com", remoteRoot: "~/work")

        store.updateSSHWorkspace(id: group.id, data: updated)

        #expect(store.groups.first?.sshData == updated)
        #expect(persistence.savedGroups?.first?.sshData == updated)
    }

    @Test("addRemoteProject appends a project and persists")
    func addRemoteProject() {
        let group = ProjectGroup(name: "Remote", type: .ssh, sshData: SSHWorkspaceData(host: "example.com", remoteRoot: "~"))
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)

        let project = store.addRemoteProject(name: "api", path: "~/code/api", toGroup: group.id)

        #expect(project != nil)
        #expect(store.groups.first?.remoteProjects.map(\.path) == ["~/code/api"])
        #expect(persistence.savedGroups?.first?.remoteProjects.count == 1)
    }

    @Test("addRemoteProject returns the existing project for a duplicate path")
    func addRemoteProjectDeduplicatesByPath() {
        let group = ProjectGroup(name: "Remote", type: .ssh, sshData: SSHWorkspaceData(host: "example.com", remoteRoot: "~"))
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)
        let first = store.addRemoteProject(name: "api", path: "~/code/api", toGroup: group.id)

        let second = store.addRemoteProject(name: "api-again", path: "~/code/./api", toGroup: group.id)

        #expect(second?.id == first?.id)
        #expect(store.groups.first?.remoteProjects.count == 1)
    }

    @Test("addRemoteProject rejects a path equal to the workspace root")
    func addRemoteProjectRejectsWorkspaceRoot() {
        let group = ProjectGroup(name: "Remote", type: .ssh, sshData: SSHWorkspaceData(host: "example.com", remoteRoot: "~/code"))
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)

        let project = store.addRemoteProject(name: "root", path: "~/code", toGroup: group.id)

        #expect(project == nil)
        #expect(store.groups.first?.remoteProjects.isEmpty == true)
    }

    @Test("removeRemoteProject deletes the project and persists")
    func removeRemoteProject() {
        let group = ProjectGroup(name: "Remote", type: .ssh, sshData: SSHWorkspaceData(host: "example.com", remoteRoot: "~"))
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)
        let project = store.addRemoteProject(name: "api", path: "~/code/api", toGroup: group.id)

        store.removeRemoteProject(id: project!.id, fromGroup: group.id)

        #expect(store.groups.first?.remoteProjects.isEmpty == true)
        #expect(persistence.savedGroups?.first?.remoteProjects.isEmpty == true)
    }

    @Test("renameRemoteProject updates the name and persists")
    func renameRemoteProject() {
        let group = ProjectGroup(name: "Remote", type: .ssh, sshData: SSHWorkspaceData(host: "example.com", remoteRoot: "~"))
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)
        let project = store.addRemoteProject(name: "api", path: "~/code/api", toGroup: group.id)

        store.renameRemoteProject(id: project!.id, to: "service")

        #expect(store.groups.first?.remoteProjects.first?.name == "service")
        #expect(persistence.savedGroups?.first?.remoteProjects.first?.name == "service")
    }

    @Test("setRemoteProjectWorktreesEnabled toggles the flag and persists")
    func setRemoteProjectWorktreesEnabled() {
        let group = ProjectGroup(name: "Remote", type: .ssh, sshData: SSHWorkspaceData(host: "example.com", remoteRoot: "~"))
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)
        let project = store.addRemoteProject(name: "api", path: "~/code/api", toGroup: group.id)

        store.setRemoteProjectWorktreesEnabled(id: project!.id, to: true)

        #expect(store.groups.first?.remoteProjects.first?.worktreesEnabled == true)
        #expect(persistence.savedGroups?.first?.remoteProjects.first?.worktreesEnabled == true)
    }

    @Test("RemoteProject.asProject preserves the worktrees flag and workspace id")
    func remoteProjectAsProjectRoundTrip() {
        let workspaceID = UUID()
        let remote = RemoteProject(name: "api", path: "~/code/api", worktreesEnabled: true)

        let project = remote.asProject(workspaceID: workspaceID, sortOrder: 3)

        #expect(project.id == remote.id)
        #expect(project.worktreesEnabled == true)
        #expect(project.remoteWorkspaceID == workspaceID)
        #expect(project.sortOrder == 3)
    }
}

final class ProjectGroupPersistenceStub: ProjectGroupPersisting {
    var groups: [ProjectGroup]
    var savedGroups: [ProjectGroup]?
    var storedActiveGroupID: UUID?

    init(initial: [ProjectGroup] = [], storedActiveGroupID: UUID? = nil) {
        groups = initial
        self.storedActiveGroupID = storedActiveGroupID
    }

    func loadProjectGroups() throws -> [ProjectGroup] {
        groups
    }

    func saveProjectGroups(_ groups: [ProjectGroup]) throws {
        savedGroups = groups
        self.groups = groups
    }

    func loadActiveGroupID() -> UUID? {
        storedActiveGroupID
    }

    func saveActiveGroupID(_ id: UUID?) {
        storedActiveGroupID = id
    }
}
