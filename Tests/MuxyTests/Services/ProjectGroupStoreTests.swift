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

    @Test("moveProject transfers projectID between groups and persists")
    func moveProject() {
        let projectID = UUID()
        let source = ProjectGroup(name: "Work", sortOrder: 0, projectIDs: [projectID])
        let destination = ProjectGroup(name: "Personal", sortOrder: 1)
        let persistence = ProjectGroupPersistenceStub(initial: [source, destination])
        let store = ProjectGroupStore(persistence: persistence)

        store.moveProject(projectID: projectID, fromGroup: source.id, toGroup: destination.id)

        let updatedSource = store.groups.first(where: { $0.id == source.id })
        let updatedDestination = store.groups.first(where: { $0.id == destination.id })
        #expect(updatedSource?.projectIDs.isEmpty == true)
        #expect(updatedDestination?.projectIDs == [projectID])
    }

    @Test("moveProject with same source and destination is a no-op")
    func moveProjectSameGroup() {
        let projectID = UUID()
        let group = ProjectGroup(name: "Work", projectIDs: [projectID])
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)

        store.moveProject(projectID: projectID, fromGroup: group.id, toGroup: group.id)

        #expect(store.groups.first?.projectIDs == [projectID])
    }

    @Test("reorderGroups updates sortOrder and persists")
    func reorderGroups() {
        let first = ProjectGroup(name: "A", sortOrder: 0)
        let second = ProjectGroup(name: "B", sortOrder: 1)
        let persistence = ProjectGroupPersistenceStub(initial: [first, second])
        let store = ProjectGroupStore(persistence: persistence)

        store.reorderGroups(fromOffsets: IndexSet(integer: 0), toOffset: 2)

        #expect(store.groups.first?.name == "B")
        #expect(store.groups.last?.name == "A")
        #expect(persistence.savedGroups?.first?.name == "B")
    }

    @Test("reorderProjects moves projectIDs within group and persists")
    func reorderProjects() {
        let idA = UUID()
        let idB = UUID()
        let group = ProjectGroup(name: "Work", projectIDs: [idA, idB])
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)

        store.reorderProjects(inGroup: group.id, fromOffsets: IndexSet(integer: 0), toOffset: 2)

        #expect(store.groups.first?.projectIDs == [idB, idA])
        #expect(persistence.savedGroups?.first?.projectIDs == [idB, idA])
    }

    @Test("toggleExpanded flips isExpanded and persists")
    func toggleExpanded() {
        let group = ProjectGroup(name: "Work", isExpanded: true)
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)

        store.toggleExpanded(groupID: group.id)

        #expect(store.groups.first?.isExpanded == false)
        #expect(persistence.savedGroups?.first?.isExpanded == false)

        store.toggleExpanded(groupID: group.id)

        #expect(store.groups.first?.isExpanded == true)
    }

    @Test("toggleExpanded with unknown id is a no-op")
    func toggleExpandedUnknownID() {
        let group = ProjectGroup(name: "Work", isExpanded: true)
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)

        store.toggleExpanded(groupID: UUID())

        #expect(store.groups.first?.isExpanded == true)
    }

    @Test("migration creates Default group when store is empty and projects exist")
    func migrationCreatesDefaultGroup() {
        let projectIDs = [UUID(), UUID()]
        let persistence = ProjectGroupPersistenceStub(initial: [])
        let store = ProjectGroupStore(persistence: persistence, existingProjectIDs: projectIDs)

        #expect(store.groups.count == 1)
        #expect(store.groups.first?.name == "Default")
        #expect(store.groups.first?.projectIDs == projectIDs)
        #expect(persistence.savedGroups?.count == 1)
    }

    @Test("migration does not create Default group when projects are empty")
    func migrationSkipsWhenNoProjects() {
        let persistence = ProjectGroupPersistenceStub(initial: [])
        let store = ProjectGroupStore(persistence: persistence, existingProjectIDs: [])

        #expect(store.groups.isEmpty)
    }

    @Test("migration does not create Default group when groups already exist")
    func migrationSkipsWhenGroupsExist() {
        let existingGroup = ProjectGroup(name: "Existing")
        let persistence = ProjectGroupPersistenceStub(initial: [existingGroup])
        let store = ProjectGroupStore(persistence: persistence, existingProjectIDs: [UUID()])

        #expect(store.groups.count == 1)
        #expect(store.groups.first?.name == "Existing")
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
}

private final class ProjectGroupPersistenceStub: ProjectGroupPersisting {
    var groups: [ProjectGroup]
    var savedGroups: [ProjectGroup]?

    init(initial: [ProjectGroup] = []) {
        groups = initial
    }

    func loadGroups() throws -> [ProjectGroup] {
        groups
    }

    func saveGroups(_ groups: [ProjectGroup]) throws {
        savedGroups = groups
    }
}
