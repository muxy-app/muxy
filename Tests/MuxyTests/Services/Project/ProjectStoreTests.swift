import Foundation
import Testing

@testable import Muxy

@Suite("ProjectStore")
@MainActor
struct ProjectStoreTests {
    @Test("setPreferredWorktreeParentPath persists normalized path")
    func setPreferredWorktreeParentPath() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let persistence = ProjectPersistenceStub(initial: [project])
        let store = ProjectStore(persistence: persistence)

        store.setPreferredWorktreeParentPath(id: project.id, to: " ~/worktrees ")

        let stored = store.storedProjects.first { $0.id == project.id }
        #expect(stored?.preferredWorktreeParentPath == NSString(string: "~/worktrees").expandingTildeInPath)
        #expect(persistence.projects.first?.preferredWorktreeParentPath == NSString(string: "~/worktrees").expandingTildeInPath)
    }

    @Test("setPreferredWorktreeParentPath clears empty path")
    func clearPreferredWorktreeParentPath() {
        var project = Project(name: "Repo", path: "/tmp/repo")
        project.preferredWorktreeParentPath = "/tmp/worktrees"
        let persistence = ProjectPersistenceStub(initial: [project])
        let store = ProjectStore(persistence: persistence)

        store.setPreferredWorktreeParentPath(id: project.id, to: " ")

        let stored = store.storedProjects.first { $0.id == project.id }
        #expect(stored?.preferredWorktreeParentPath == nil)
        #expect(persistence.projects.first?.preferredWorktreeParentPath == nil)
    }

    @Test("setWorktreesEnabled persists the new value")
    func setWorktreesEnabled() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let persistence = ProjectPersistenceStub(initial: [project])
        let store = ProjectStore(persistence: persistence)

        store.setWorktreesEnabled(id: project.id, to: true)

        #expect(store.storedProjects.first { $0.id == project.id }?.worktreesEnabled == true)
        #expect(persistence.projects.first?.worktreesEnabled == true)
    }

    @Test("setPinned persists the new value")
    func setPinned() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let persistence = ProjectPersistenceStub(initial: [project])
        let store = ProjectStore(persistence: persistence)

        store.setPinned(id: project.id, to: true)

        #expect(store.storedProjects.first { $0.id == project.id }?.isPinned == true)
        #expect(persistence.projects.first?.isPinned == true)
    }

    @Test("setPinned ignores the Home project")
    func setPinnedIgnoresHome() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let persistence = ProjectPersistenceStub(initial: [project])
        let store = ProjectStore(persistence: persistence)

        store.setPinned(id: Project.homeID, to: true)

        #expect(store.storedProjects.allSatisfy { !$0.isPinned })
    }

    @Test("projects always exposes Home at the front without persisting it")
    func projectsSynthesizesHome() {
        let existing = Project(name: "Repo", path: "/tmp/repo")
        let persistence = ProjectPersistenceStub(initial: [existing])
        let store = ProjectStore(persistence: persistence)

        #expect(store.projects.first?.isHome == true)
        #expect(store.projects.count == 2)
        #expect(store.storedProjects.contains(where: { $0.isHome }) == false)
        #expect(persistence.projects.contains(where: { $0.isHome }) == false)
    }

    @Test("load drops any persisted Home record")
    func loadDropsPersistedHome() {
        let persistence = ProjectPersistenceStub(initial: [Project.home, Project(name: "Repo", path: "/tmp/repo")])
        let store = ProjectStore(persistence: persistence)

        #expect(store.storedProjects.contains(where: { $0.isHome }) == false)
        #expect(store.projects.filter(\.isHome).count == 1)
    }

    @Test("remove never deletes the Home project")
    func removeIgnoresHome() {
        let persistence = ProjectPersistenceStub(initial: [])
        let store = ProjectStore(persistence: persistence)

        store.remove(id: Project.homeID)

        #expect(store.projects.contains { $0.isHome })
    }

    @Test("markActive stamps lastActiveAt and persists")
    func markActiveStampsTimestamp() {
        let project = Project(name: "Repo", path: "/tmp/repo")
        let persistence = ProjectPersistenceStub(initial: [project])
        let store = ProjectStore(persistence: persistence)

        #expect(store.storedProjects.first?.lastActiveAt == nil)

        store.markActive(id: project.id)

        #expect(store.storedProjects.first?.lastActiveAt != nil)
        #expect(persistence.projects.first?.lastActiveAt != nil)
    }

    @Test("markActive ignores unknown ids")
    func markActiveIgnoresUnknown() {
        let persistence = ProjectPersistenceStub(initial: [Project(name: "Repo", path: "/tmp/repo")])
        let store = ProjectStore(persistence: persistence)

        store.markActive(id: UUID())

        #expect(store.storedProjects.first?.lastActiveAt == nil)
    }

    @Test("persistOrder rewrites sortOrder to match the given order")
    func persistOrderRewritesSortOrder() {
        let first = Project(name: "A", path: "/tmp/a", sortOrder: 0)
        let second = Project(name: "B", path: "/tmp/b", sortOrder: 1)
        let third = Project(name: "C", path: "/tmp/c", sortOrder: 2)
        let persistence = ProjectPersistenceStub(initial: [first, second, third])
        let store = ProjectStore(persistence: persistence)

        store.persistOrder([third.id, first.id, second.id])

        #expect(store.storedProjects.map(\.id) == [third.id, first.id, second.id])
        #expect(store.storedProjects.map(\.sortOrder) == [0, 1, 2])
        #expect(persistence.projects.map(\.id) == [third.id, first.id, second.id])
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
