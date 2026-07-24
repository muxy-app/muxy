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

    private func project(named name: String) -> Project {
        Project(name: name, path: "/tmp/\(name)")
    }
}
