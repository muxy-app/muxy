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
