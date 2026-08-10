import Foundation
import Testing

@testable import Muxy

@Suite("ProjectListSearch")
@MainActor
struct ProjectListSearchTests {
    @Test("returns all projects for an empty query")
    func emptyQueryReturnsAllProjects() {
        let projects = [project(named: "Alpha"), project(named: "Beta")]

        #expect(ProjectListSearch.filter(projects, matching: "   ") == projects)
    }

    @Test("matches project names case-insensitively")
    func matchesProjectNamesCaseInsensitively() {
        let matchingProject = project(named: "Muxy Desktop")
        let projects = [project(named: "Alpha"), matchingProject, project(named: "Beta")]

        #expect(ProjectListSearch.filter(projects, matching: "  DESKTOP  ") == [matchingProject])
    }

    @Test("matches the localized Home name without translating user project names")
    func matchesLocalizedHomeName() throws {
        let fixture = try LocalizationTestSupport.makeService(
            translations: #"""
            "Home" = "Startseite";
            """#
        )
        defer { try? FileManager.default.removeItem(at: fixture.root) }
        let userProject = project(named: "Home")

        #expect(ProjectListSearch.filter(
            [Project.home, userProject],
            matching: "startseite",
            localization: fixture.service
        ) == [Project.home])
        #expect(ProjectListSearch.filter(
            [Project.home, userProject],
            matching: "Home",
            localization: fixture.service
        ) == [Project.home, userProject])
    }

    @Test("preserves the displayed project order")
    func preservesDisplayedProjectOrder() {
        let firstMatch = project(named: "API")
        let secondMatch = project(named: "API Docs")
        let projects = [firstMatch, project(named: "Client"), secondMatch]

        #expect(ProjectListSearch.filter(projects, matching: "api") == [firstMatch, secondMatch])
    }

    @Test("returns no projects when nothing matches")
    func returnsNoProjectsWhenNothingMatches() {
        let projects = [project(named: "Alpha"), project(named: "Beta")]

        #expect(ProjectListSearch.filter(projects, matching: "gamma").isEmpty)
    }

    @Test("search query is active only while the wide search field is visible")
    func searchQueryRequiresVisibleWideField() {
        #expect(ProjectListSearch.activeQuery("Alpha", isVisible: true, isWide: true) == "Alpha")
        #expect(ProjectListSearch.activeQuery("Alpha", isVisible: false, isWide: true).isEmpty)
        #expect(ProjectListSearch.activeQuery("Alpha", isVisible: true, isWide: false).isEmpty)
    }

    @Test("always show search is disabled by default")
    func alwaysShowSearchIsDisabledByDefault() {
        #expect(!ProjectSearchPreferences.defaultVisible)
    }

    @Test("filtered projects keep their navigation shortcut index")
    func filteredProjectsKeepNavigationShortcutIndex() {
        let alpha = project(named: "Alpha")
        let beta = project(named: "Beta")
        let navigationOrder = ProjectNavigationOrder.projects(
            homeProject: Project.home,
            displayedProjects: [alpha, beta]
        )
        let filteredProjects = ProjectListSearch.filter([alpha, beta], matching: "beta")
        let shortcutIndices = ProjectNavigationOrder.shortcutIndices(in: navigationOrder)

        #expect(filteredProjects.count == 1)
        #expect(shortcutIndices[filteredProjects[0].id] == 3)
    }

    @Test("projects after the ninth navigation target have no shortcut")
    func projectsAfterNinthNavigationTargetHaveNoShortcut() {
        let projects = (1 ... 10).map { project(named: "Project \($0)") }
        let shortcutIndices = ProjectNavigationOrder.shortcutIndices(in: projects)

        #expect(shortcutIndices[projects[8].id] == 9)
        #expect(shortcutIndices[projects[9].id] == nil)
    }

    private func project(named name: String) -> Project {
        Project(name: name, path: "/tmp/\(name)")
    }
}
