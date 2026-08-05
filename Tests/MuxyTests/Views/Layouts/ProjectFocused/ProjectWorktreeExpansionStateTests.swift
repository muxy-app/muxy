import Foundation
import SwiftUI
import Testing

@testable import Muxy

@Suite("ProjectWorktreeExpansionState")
@MainActor
struct ProjectWorktreeExpansionStateTests {
    @Test("project row forwards expansion changes to its parent")
    func projectRowForwardsExpansionChanges() {
        var expanded = false
        let row = ExpandedProjectRow(
            project: project(named: "Alpha"),
            shortcutIndex: nil,
            isAnyDragging: false,
            worktreesExpanded: Binding(
                get: { expanded },
                set: { expanded = $0 }
            ),
            onSelect: {},
            onRemove: {},
            onRename: { _ in },
            onSetLogo: { _ in },
            onSetIcon: { _ in },
            onSetIconColor: { _ in },
            onSetWorktreesEnabled: { _ in },
            onSetPinned: { _ in }
        )

        row.worktreesExpanded = true

        #expect(expanded)
    }

    @Test("preserves expansion while search filters out a project")
    func preservesExpansionWhileSearchFiltersProject() {
        let alpha = project(named: "Alpha")
        let beta = project(named: "Beta")
        let projects = [alpha, beta]
        var state = ProjectWorktreeExpansionState()
        state[alpha.id] = true

        let filteredProjects = ProjectListSearch.filter(projects, matching: "Beta")
        let restoredProjects = ProjectListSearch.filter(projects, matching: "")

        #expect(filteredProjects == [beta])
        #expect(restoredProjects == projects)
        #expect(state[alpha.id])
        #expect(!state[beta.id])
    }

    @Test("preserves all expansions through a search with no results")
    func preservesExpansionsThroughNoResultsSearch() {
        let alpha = project(named: "Alpha")
        let beta = project(named: "Beta")
        var state = ProjectWorktreeExpansionState()
        state[alpha.id] = true
        state[beta.id] = true

        let filteredProjects = ProjectListSearch.filter([alpha, beta], matching: "Gamma")

        #expect(filteredProjects.isEmpty)
        #expect(state[alpha.id])
        #expect(state[beta.id])
    }

    @Test("preserves expansion changes made while search is active")
    func preservesChangesMadeWhileSearchIsActive() {
        let alpha = project(named: "Alpha")
        let beta = project(named: "Beta")
        let projects = [alpha, beta]
        var state = ProjectWorktreeExpansionState()
        state[alpha.id] = true
        state[beta.id] = true

        let filteredProjects = ProjectListSearch.filter(projects, matching: "Beta")
        state[beta.id] = false
        let restoredProjects = ProjectListSearch.filter(projects, matching: "")

        #expect(filteredProjects == [beta])
        #expect(restoredProjects == projects)
        #expect(state[alpha.id])
        #expect(!state[beta.id])
    }

    @Test("reconciles expansion against navigation projects instead of search results")
    func reconcilesExpansionAgainstNavigationProjects() {
        let alpha = project(named: "Alpha")
        let beta = project(named: "Beta")
        let projects = [alpha, beta]
        var state = ProjectWorktreeExpansionState()
        state[alpha.id] = true
        state[beta.id] = true

        let filteredProjects = ProjectListSearch.filter(projects, matching: "Beta")
        let navigationProjects = ProjectNavigationOrder.projects(homeProject: nil, displayedProjects: projects)
        state.retain(projectIDs: Set(navigationProjects.map(\.id)))

        #expect(filteredProjects == [beta])
        #expect(state[alpha.id])
        #expect(state[beta.id])
    }

    @Test("discards expansion when a project leaves the sidebar")
    func discardsExpansionForRemovedProject() {
        let removedProjectID = UUID()
        let remainingProjectID = UUID()
        var state = ProjectWorktreeExpansionState()
        state[removedProjectID] = true
        state[remainingProjectID] = true

        state.retain(projectIDs: [remainingProjectID])

        #expect(!state[removedProjectID])
        #expect(state[remainingProjectID])
    }

    private func project(named name: String) -> Project {
        Project(name: name, path: "/tmp/\(name)")
    }
}
