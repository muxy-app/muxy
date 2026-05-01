import Foundation
import Testing

@testable import Muxy

@Suite("WorktreeLocationResolver")
struct WorktreeLocationResolverTests {
    @Test("project location wins over global default")
    func projectLocationWins() {
        var project = Project(name: "Repo", path: "/tmp/repo")
        project.preferredWorktreeParentPath = "/tmp/project-worktrees"

        let path = WorktreeLocationResolver.worktreeDirectory(
            for: project,
            slug: "feature-a",
            defaultParentPath: "/tmp/global-worktrees"
        )

        #expect(path == "/tmp/project-worktrees/feature-a")
    }

    @Test("global default groups worktrees by project name")
    func globalDefaultGroupsByProjectName() {
        let project = Project(name: "My Repo", path: "/tmp/repo")

        let path = WorktreeLocationResolver.worktreeDirectory(
            for: project,
            slug: "feature-a",
            defaultParentPath: "/tmp/global-worktrees"
        )

        #expect(path == "/tmp/global-worktrees/My-Repo/feature-a")
    }

    @Test("missing settings fall back to app support")
    func missingSettingsFallback() {
        let project = Project(name: "Repo", path: "/tmp/repo")

        let path = WorktreeLocationResolver.worktreeDirectory(
            for: project,
            slug: "feature-a",
            defaultParentPath: nil
        )

        let expected = MuxyFileStorage.worktreeRoot(forProjectID: project.id, create: false)
            .appendingPathComponent("feature-a", isDirectory: true)
            .path
        #expect(path == expected)
    }
}
