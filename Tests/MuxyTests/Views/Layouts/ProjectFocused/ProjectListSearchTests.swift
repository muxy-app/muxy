import Testing

@testable import Muxy

@Suite("ProjectListSearch")
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

    @Test("search field is hidden by default")
    func searchFieldIsHiddenByDefault() {
        #expect(!ProjectSearchPreferences.defaultVisible)
    }

    private func project(named name: String) -> Project {
        Project(name: name, path: "/tmp/\(name)")
    }
}
