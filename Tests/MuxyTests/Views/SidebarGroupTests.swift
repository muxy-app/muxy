import Foundation
import Testing

@testable import Muxy

@Suite("Sidebar group integration")
@MainActor
struct SidebarGroupTests {
    @Test("removeProjectFromAllGroups removes project from every group that contains it")
    func removeProjectFromAllGroups() {
        let projectID = UUID()
        let groupA = ProjectGroup(name: "A", projectIDs: [projectID])
        let groupB = ProjectGroup(name: "B", projectIDs: [UUID()])
        let persistence = ProjectGroupPersistenceStub(initial: [groupA, groupB])
        let store = ProjectGroupStore(persistence: persistence)

        store.removeProjectFromAllGroups(projectID: projectID)

        #expect(store.groups.first(where: { $0.id == groupA.id })?.projectIDs.isEmpty == true)
        #expect(store.groups.first(where: { $0.id == groupB.id })?.projectIDs.count == 1)
    }

    @Test("removeProjectFromAllGroups on unknown projectID is a no-op")
    func removeProjectFromAllGroupsUnknown() {
        let projectID = UUID()
        let group = ProjectGroup(name: "A", projectIDs: [projectID])
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)

        store.removeProjectFromAllGroups(projectID: UUID())

        #expect(store.groups.first?.projectIDs == [projectID])
    }

    @Test("addProjectToFirstGroupOrDefault adds to first group when groups exist")
    func addProjectToFirstGroupWhenGroupsExist() {
        let newProjectID = UUID()
        let group = ProjectGroup(name: "Default")
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)

        store.addProjectToFirstGroupOrDefault(projectID: newProjectID)

        #expect(store.groups.first?.projectIDs == [newProjectID])
        #expect(persistence.savedGroups?.first?.projectIDs == [newProjectID])
    }

    @Test("addProjectToFirstGroupOrDefault creates Default group when no groups exist")
    func addProjectToFirstGroupCreatesDefault() {
        let newProjectID = UUID()
        let persistence = ProjectGroupPersistenceStub(initial: [])
        let store = ProjectGroupStore(persistence: persistence)

        store.addProjectToFirstGroupOrDefault(projectID: newProjectID)

        #expect(store.groups.count == 1)
        #expect(store.groups.first?.name == "Default")
        #expect(store.groups.first?.projectIDs == [newProjectID])
    }

    @Test("ProjectStore onProjectAdded callback fires after add")
    func projectStoreAddCallbackFires() {
        let project = Project(name: "Test", path: "/tmp/test")
        let persistence = ProjectPersistenceStub(initial: [])
        let store = ProjectStore(persistence: persistence)
        var received: Project?

        store.onProjectAdded = { received = $0 }
        store.add(project)

        #expect(received?.id == project.id)
    }

    @Test("ProjectStore onProjectRemoved callback fires after remove")
    func projectStoreRemoveCallbackFires() {
        let project = Project(name: "Test", path: "/tmp/test")
        let persistence = ProjectPersistenceStub(initial: [project])
        let store = ProjectStore(persistence: persistence)
        var receivedID: UUID?

        store.onProjectRemoved = { receivedID = $0 }
        store.remove(id: project.id)

        #expect(receivedID == project.id)
    }

    @Test("ProjectStore onProjectAdded is nil-safe when no callback set")
    func projectStoreAddNoCallback() {
        let project = Project(name: "Test", path: "/tmp/test")
        let persistence = ProjectPersistenceStub(initial: [])
        let store = ProjectStore(persistence: persistence)

        store.add(project)

        #expect(store.projects.count == 1)
    }

    @Test("ProjectStore onProjectRemoved is nil-safe when no callback set")
    func projectStoreRemoveNoCallback() {
        let project = Project(name: "Test", path: "/tmp/test")
        let persistence = ProjectPersistenceStub(initial: [project])
        let store = ProjectStore(persistence: persistence)

        store.remove(id: project.id)

        #expect(store.projects.isEmpty)
    }

    @Test("ProjectGroupStore init with existing IDs triggers migration to Default group")
    func initWithExistingIDsCreatesMigration() {
        let ids = [UUID(), UUID(), UUID()]
        let persistence = ProjectGroupPersistenceStub(initial: [])
        let store = ProjectGroupStore(persistence: persistence, existingProjectIDs: ids)

        #expect(store.groups.count == 1)
        #expect(store.groups.first?.name == "Default")
        #expect(store.groups.first?.projectIDs == ids)
    }

    @Test("removeProjectFromAllGroups persists changes")
    func removeProjectFromAllGroupsPersists() {
        let projectID = UUID()
        let group = ProjectGroup(name: "Work", projectIDs: [projectID])
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)

        store.removeProjectFromAllGroups(projectID: projectID)

        #expect(persistence.savedGroups?.first?.projectIDs.isEmpty == true)
    }

    @Test("addProjectToFirstGroupOrDefault persists Default group creation")
    func addProjectPersistsDefaultGroupCreation() {
        let newProjectID = UUID()
        let persistence = ProjectGroupPersistenceStub(initial: [])
        let store = ProjectGroupStore(persistence: persistence)

        store.addProjectToFirstGroupOrDefault(projectID: newProjectID)

        #expect(persistence.savedGroups?.count == 1)
        #expect(persistence.savedGroups?.first?.name == "Default")
        #expect(persistence.savedGroups?.first?.projectIDs == [newProjectID])
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
        self.groups = groups
    }
}

private final class ProjectPersistenceStub: ProjectPersisting {
    var projects: [Project]

    init(initial: [Project]) {
        projects = initial
    }

    func loadProjects() throws -> [Project] {
        projects
    }

    func saveProjects(_ projects: [Project]) throws {
        self.projects = projects
    }
}
