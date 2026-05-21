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

    @Test("addProject ignores duplicate projectID")
    func addProjectDuplicate() {
        let projectID = UUID()
        let group = ProjectGroup(name: "Work", projectIDs: [projectID])
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)

        store.addProject(projectID: projectID, toGroup: group.id)

        #expect(store.groups.first?.projectIDs.count == 1)
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
