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

        #expect(store.projects.first?.preferredWorktreeParentPath == NSString(string: "~/worktrees").expandingTildeInPath)
        #expect(persistence.projects.first?.preferredWorktreeParentPath == NSString(string: "~/worktrees").expandingTildeInPath)
    }

    @Test("setPreferredWorktreeParentPath clears empty path")
    func clearPreferredWorktreeParentPath() {
        var project = Project(name: "Repo", path: "/tmp/repo")
        project.preferredWorktreeParentPath = "/tmp/worktrees"
        let persistence = ProjectPersistenceStub(initial: [project])
        let store = ProjectStore(persistence: persistence)

        store.setPreferredWorktreeParentPath(id: project.id, to: " ")

        #expect(store.projects.first?.preferredWorktreeParentPath == nil)
        #expect(persistence.projects.first?.preferredWorktreeParentPath == nil)
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
