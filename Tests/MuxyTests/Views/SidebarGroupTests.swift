import Foundation
import Testing

@testable import Muxy

@Suite("Sidebar project group integration")
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

    @Test("removeProjectFromAllGroups persists changes")
    func removeProjectFromAllGroupsPersists() {
        let projectID = UUID()
        let group = ProjectGroup(name: "Work", projectIDs: [projectID])
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)

        store.removeProjectFromAllGroups(projectID: projectID)

        #expect(persistence.savedGroups?.first?.projectIDs.isEmpty == true)
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

    @Test("ProjectStore onProjectRemoved is nil-safe when no callback set")
    func projectStoreRemoveNoCallback() {
        let project = Project(name: "Test", path: "/tmp/test")
        let persistence = ProjectPersistenceStub(initial: [project])
        let store = ProjectStore(persistence: persistence)

        store.remove(id: project.id)

        #expect(store.projects.isEmpty)
    }

    @Test("group selection survives unrelated group renames")
    func groupSelectionSurvivesUnrelatedChanges() {
        let groupA = ProjectGroup(name: "A")
        let groupB = ProjectGroup(name: "B")
        let persistence = ProjectGroupPersistenceStub(initial: [groupA, groupB])
        let store = ProjectGroupStore(persistence: persistence)
        store.selectGroup(id: groupA.id)

        store.renameGroup(id: groupB.id, to: "B Renamed")

        #expect(store.activeGroupID == groupA.id)
    }

    @Test("addProject removes project from other groups (single-membership)")
    func addProjectIsExclusive() {
        let projectID = UUID()
        let groupA = ProjectGroup(name: "A", projectIDs: [projectID])
        let groupB = ProjectGroup(name: "B")
        let persistence = ProjectGroupPersistenceStub(initial: [groupA, groupB])
        let store = ProjectGroupStore(persistence: persistence)

        store.addProject(projectID: projectID, toGroup: groupB.id)

        #expect(store.groups.first(where: { $0.id == groupA.id })?.projectIDs.contains(projectID) == false)
        #expect(store.groups.first(where: { $0.id == groupB.id })?.projectIDs.contains(projectID) == true)
    }

    @Test("filteredProjects after group selection only shows group projects")
    func filteredProjectsAfterSelection() {
        let projectA = Project(name: "A", path: "/a")
        let projectB = Project(name: "B", path: "/b")
        let group = ProjectGroup(name: "Work", projectIDs: [projectA.id])
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)

        store.selectGroup(id: group.id)
        let filtered = store.filteredProjects(from: [projectA, projectB])

        #expect(filtered.count == 1)
        #expect(filtered.first?.id == projectA.id)
    }

    @Test("filteredProjects after clearGroupSelection shows all projects")
    func filteredProjectsAfterClearSelection() {
        let projectA = Project(name: "A", path: "/a")
        let projectB = Project(name: "B", path: "/b")
        let group = ProjectGroup(name: "Work", projectIDs: [projectA.id])
        let persistence = ProjectGroupPersistenceStub(initial: [group])
        let store = ProjectGroupStore(persistence: persistence)

        store.selectGroup(id: group.id)
        store.clearGroupSelection()
        let filtered = store.filteredProjects(from: [projectA, projectB])

        #expect(filtered.count == 2)
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
