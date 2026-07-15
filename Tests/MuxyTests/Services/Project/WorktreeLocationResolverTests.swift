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
            branch: "feature/a",
            defaultPathTemplate: nil,
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
            branch: "feature/a",
            defaultPathTemplate: nil,
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
            branch: "feature/a",
            defaultPathTemplate: nil,
            defaultParentPath: nil
        )

        let expected = MuxyFileStorage.worktreeRoot(forProjectID: project.id, create: false)
            .appendingPathComponent("feature-a", isDirectory: true)
            .path
        #expect(path == expected)
    }

    @Test("relative template replaces project and branch variables")
    func relativeProjectAndBranchTemplate() {
        var project = Project(name: "My Repo", path: "/tmp/checkouts/my-app")
        project.preferredWorktreePathTemplate = "../{project-name}.{branch}"

        let path = WorktreeLocationResolver.worktreeDirectory(
            for: project,
            slug: "ignored-name",
            branch: "feature/auth",
            defaultPathTemplate: nil,
            defaultParentPath: nil
        )

        #expect(path == "/tmp/checkouts/My-Repo.feature-auth")
    }

    @Test("base directory variable uses the checkout folder instead of project name")
    func baseDirectoryTemplate() {
        var project = Project(name: "Renamed", path: "/tmp/checkouts/my-app")
        project.preferredWorktreePathTemplate = "../{base-dir}.{branch}"

        let path = WorktreeLocationResolver.worktreeDirectory(
            for: project,
            slug: "ignored-name",
            branch: "fix-header",
            defaultPathTemplate: nil,
            defaultParentPath: nil
        )

        #expect(path == "/tmp/checkouts/my-app.fix-header")
    }

    @Test("relative template supports a sibling worktrees folder")
    func siblingWorktreesTemplate() {
        var project = Project(name: "My App", path: "/tmp/checkouts/my-app")
        project.preferredWorktreePathTemplate = "../worktrees/{project-name}{branch}"

        let path = WorktreeLocationResolver.worktreeDirectory(
            for: project,
            slug: "ignored-name",
            branch: "fix/header",
            defaultPathTemplate: nil,
            defaultParentPath: nil
        )

        #expect(path == "/tmp/checkouts/worktrees/My-Appfix-header")
    }

    @Test("absolute and home-relative templates remain rooted outside the project")
    func rootedTemplates() {
        let project = Project(name: "Repo", path: "/tmp/checkouts/repo")

        let absolutePath = WorktreeLocationResolver.worktreeDirectory(
            for: project,
            slug: "ignored-name",
            branch: "feature/a",
            defaultPathTemplate: "/var/tmp/{base-dir}.{branch}",
            defaultParentPath: nil
        )
        let homePath = WorktreeLocationResolver.worktreeDirectory(
            for: project,
            slug: "ignored-name",
            branch: "feature/a",
            defaultPathTemplate: "~/.worktrees/{branch}",
            defaultParentPath: nil
        )

        #expect(absolutePath == "/var/tmp/repo.feature-a")
        #expect(homePath == FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".worktrees/feature-a").path)
    }

    @Test("project template wins over every global and legacy location")
    func projectTemplateWins() {
        var project = Project(name: "Repo", path: "/tmp/checkouts/repo")
        project.preferredWorktreePathTemplate = "../project-{branch}"
        project.preferredWorktreeParentPath = "/tmp/project-parent"

        let path = WorktreeLocationResolver.worktreeDirectory(
            for: project,
            slug: "feature-a",
            branch: "feature/a",
            defaultPathTemplate: "/tmp/global-{branch}",
            defaultParentPath: "/tmp/global-parent"
        )

        #expect(path == "/tmp/checkouts/project-feature-a")
    }

    @Test("global template wins over legacy global parent folder")
    func globalTemplateWins() {
        let project = Project(name: "Repo", path: "/tmp/checkouts/repo")

        let path = WorktreeLocationResolver.worktreeDirectory(
            for: project,
            slug: "feature-a",
            branch: "feature/a",
            defaultPathTemplate: "../global-{branch}",
            defaultParentPath: "/tmp/global-parent"
        )

        #expect(path == "/tmp/checkouts/global-feature-a")
    }

    @Test("template variables cannot add path separators or traversal components")
    func templateVariablesAreSafePathComponents() {
        var project = Project(name: "My/Repo", path: "/tmp/checkouts/repo")
        project.preferredWorktreePathTemplate = "../{project-name}/{branch}"

        let path = WorktreeLocationResolver.worktreeDirectory(
            for: project,
            slug: "ignored-name",
            branch: "feature/auth",
            defaultPathTemplate: nil,
            defaultParentPath: nil
        )

        #expect(path == "/tmp/checkouts/My-Repo/feature-auth")
        #expect(WorktreeLocationResolver.sanitizedDirectoryName(from: "..") == "project")
    }

    @Test("remote projects keep their fixed remote worktree layout")
    func remoteProjectKeepsExistingLayout() {
        var project = Project(
            name: "My Repo",
            path: "/srv/my-repo",
            remoteWorkspaceID: UUID()
        )
        project.preferredWorktreePathTemplate = "../{base-dir}.{branch}"

        let path = WorktreeLocationResolver.worktreeDirectory(
            for: project,
            slug: "feature-a",
            branch: "feature/a",
            defaultPathTemplate: "../global-{branch}",
            defaultParentPath: "/tmp/global-parent"
        )

        #expect(path == "/srv/.muxy-worktrees/My-Repo/feature-a")
    }
}
