import AppKit
import Foundation
import SwiftUI
import Testing

@testable import Muxy

@Suite("ProjectWorktreeExpansionState", .serialized)
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
            isWorktreeAutoExpandPending: false,
            onResolveWorktreeAutoExpand: { _ in },
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

    @Test("initializes expansion only when a project has no preserved state")
    func initializesExpansionOnlyOnce() {
        let projectID = UUID()
        var state = ProjectWorktreeExpansionState()

        state.initializeExpanded(projectID: projectID)
        #expect(state[projectID])

        state[projectID] = false
        state.initializeExpanded(projectID: projectID)

        #expect(!state[projectID])
    }

    @Test("applies each project switch auto-expand request once")
    func appliesProjectSwitchAutoExpandRequestOnce() {
        let projectID = UUID()
        var state = ProjectWorktreeExpansionState()
        state[projectID] = false

        state.requestAutoExpand(projectID: projectID)
        #expect(state.hasPendingAutoExpandRequest(projectID: projectID))

        state.resolveAutoExpand(projectID: projectID, isEligible: true)
        #expect(state[projectID])
        #expect(!state.hasPendingAutoExpandRequest(projectID: projectID))

        state[projectID] = false
        state.resolveAutoExpand(projectID: projectID, isEligible: true)
        #expect(!state[projectID])
    }

    @Test("cancels an ineligible project switch auto-expand request")
    func cancelsIneligibleProjectSwitchAutoExpandRequest() {
        let projectID = UUID()
        var state = ProjectWorktreeExpansionState()
        state[projectID] = false
        state.requestAutoExpand(projectID: projectID)

        state.resolveAutoExpand(projectID: projectID, isEligible: false)

        #expect(!state[projectID])
        #expect(!state.hasPendingAutoExpandRequest(projectID: projectID))
        state.resolveAutoExpand(projectID: projectID, isEligible: true)
        #expect(!state[projectID])
    }

    @Test("preserves explicit collapse when search recreates an auto-expanding row")
    func preservesExplicitCollapseWhenSearchRecreatesRow() async throws {
        let project = worktreeProject()
        let environment = ProjectManagementEnvironment(projects: [project])
        environment.appState.activeProjectID = project.id
        let model = ProjectWorktreeExpansionLifecycleModel()
        let restoreAutoExpand = enableAutoExpand(for: project, environment: environment)
        defer { restoreAutoExpand() }
        let (hostingView, window) = hostLifecycleHarness(
            project: project,
            model: model,
            environment: environment
        )
        defer { window.orderOut(nil) }

        try await waitUntil(hostingView: hostingView) {
            model.autoExpansionAttempts == 1 && model.expansion[project.id]
        }

        model.expansion[project.id] = false
        model.isRowVisible = false
        try await waitUntil(hostingView: hostingView) {
            model.rowDisappearances == 1
        }

        model.isRowVisible = true
        try await waitUntil(hostingView: hostingView) {
            model.autoExpansionAttempts == 2
        }

        #expect(!model.expansion[project.id])
    }

    @Test("auto-expands a searched-out project after a genuine project switch")
    func autoExpandsSearchedOutProjectAfterProjectSwitch() async throws {
        let activeProject = project(named: "Initially Active")
        let searchedOutProject = worktreeProject()
        let environment = ProjectManagementEnvironment(projects: [activeProject, searchedOutProject])
        environment.appState.activeProjectID = activeProject.id
        let model = ProjectWorktreeExpansionLifecycleModel()
        model.expansion[searchedOutProject.id] = false
        model.isRowVisible = false
        let restoreAutoExpand = enableAutoExpand(for: searchedOutProject, environment: environment)
        defer { restoreAutoExpand() }
        let (hostingView, window) = hostLifecycleHarness(
            project: searchedOutProject,
            model: model,
            environment: environment
        )
        defer { window.orderOut(nil) }

        try await waitUntil(hostingView: hostingView) {
            model.isMounted
        }
        #expect(model.autoExpansionAttempts == 0)
        #expect(!model.expansion[searchedOutProject.id])

        environment.appState.activeProjectID = searchedOutProject.id
        try await waitUntil(hostingView: hostingView) {
            model.projectSwitchObservations == 1 &&
                model.expansion.hasPendingAutoExpandRequest(projectID: searchedOutProject.id)
        }
        #expect(model.autoExpansionAttempts == 0)
        #expect(!model.expansion[searchedOutProject.id])

        model.isRowVisible = true
        try await waitUntil(hostingView: hostingView) {
            model.autoExpansionAttempts == 1 && model.expansion[searchedOutProject.id]
        }
        #expect(!model.expansion.hasPendingAutoExpandRequest(projectID: searchedOutProject.id))

        model.expansion[searchedOutProject.id] = false
        model.isRowVisible = false
        try await waitUntil(hostingView: hostingView) {
            model.rowDisappearances == 1
        }

        model.isRowVisible = true
        try await waitUntil(hostingView: hostingView) {
            model.autoExpansionAttempts == 2
        }
        #expect(!model.expansion[searchedOutProject.id])
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
        state.requestAutoExpand(projectID: removedProjectID)

        state.retain(projectIDs: [remainingProjectID])

        #expect(!state[removedProjectID])
        #expect(state[remainingProjectID])
        #expect(!state.hasPendingAutoExpandRequest(projectID: removedProjectID))
    }

    private func project(named name: String) -> Project {
        Project(name: name, path: "/tmp/\(name)")
    }

    private func worktreeProject() -> Project {
        var project = Project(
            name: "Auto Expand",
            path: "/tmp/auto-expand-\(UUID().uuidString)"
        )
        project.worktreesEnabled = true
        return project
    }

    private func enableAutoExpand(
        for project: Project,
        environment: ProjectManagementEnvironment
    ) -> () -> Void {
        let defaults = UserDefaults.standard
        let settingKey = GeneralSettingsKeys.autoExpandWorktreesOnProjectSwitch
        let previousSetting = defaults.object(forKey: settingKey)
        let context = environment.projectGroupStore.workspaceContext(for: project)
        defaults.set(true, forKey: settingKey)
        GitRepoStatusCache.shared.update(path: project.path, context: context, isGitRepo: true)
        return {
            if let previousSetting {
                defaults.set(previousSetting, forKey: settingKey)
            } else {
                defaults.removeObject(forKey: settingKey)
            }
            GitRepoStatusCache.shared.remove(path: project.path, context: context)
        }
    }

    private func hostLifecycleHarness(
        project: Project,
        model: ProjectWorktreeExpansionLifecycleModel,
        environment: ProjectManagementEnvironment
    ) -> (NSHostingView<AnyView>, NSWindow) {
        let view = AnyView(
            ProjectWorktreeExpansionLifecycleHarness(
                project: project,
                model: model
            )
            .environment(environment.appState)
            .environment(environment.projectStore)
            .environment(environment.worktreeStore)
            .environment(environment.projectGroupStore)
        )
        let hostingView = NSHostingView(rootView: view)
        hostingView.frame = NSRect(x: 0, y: 0, width: 320, height: 240)
        let window = NSWindow(
            contentRect: hostingView.frame,
            styleMask: [.titled],
            backing: .buffered,
            defer: false
        )
        window.contentView = hostingView
        window.orderFront(nil)
        return (hostingView, window)
    }

    private func waitUntil(
        hostingView: NSView,
        condition: @MainActor () -> Bool
    ) async throws {
        for _ in 0..<100 {
            hostingView.layoutSubtreeIfNeeded()
            if condition() {
                return
            }
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(condition())
    }
}

@MainActor
@Observable
private final class ProjectWorktreeExpansionLifecycleModel {
    var expansion = ProjectWorktreeExpansionState()
    var autoExpansionAttempts = 0
    var isMounted = false
    var isRowVisible = true
    var projectSwitchObservations = 0
    var rowDisappearances = 0
}

private struct ProjectWorktreeExpansionLifecycleHarness: View {
    @Environment(AppState.self) private var appState
    @AppStorage(GeneralSettingsKeys.autoExpandWorktreesOnProjectSwitch)
    private var autoExpandWorktrees = false
    let project: Project
    @Bindable var model: ProjectWorktreeExpansionLifecycleModel

    var body: some View {
        Group {
            Color.clear
                .frame(width: 0, height: 0)
                .onAppear {
                    model.isMounted = true
                }
            if model.isRowVisible {
                ExpandedProjectRow(
                    project: project,
                    shortcutIndex: nil,
                    isAnyDragging: false,
                    worktreesExpanded: Binding(
                        get: { model.expansion[project.id] },
                        set: { model.expansion[project.id] = $0 }
                    ),
                    isWorktreeAutoExpandPending: model.expansion.hasPendingAutoExpandRequest(projectID: project.id),
                    onResolveWorktreeAutoExpand: { isEligible in
                        model.autoExpansionAttempts += 1
                        model.expansion.resolveAutoExpand(projectID: project.id, isEligible: isEligible)
                    },
                    onSelect: {},
                    onRemove: {},
                    onRename: { _ in },
                    onSetLogo: { _ in },
                    onSetIcon: { _ in },
                    onSetIconColor: { _ in },
                    onSetWorktreesEnabled: { _ in },
                    onSetPinned: { _ in }
                )
                .onDisappear {
                    model.rowDisappearances += 1
                }
            }
        }
        .onChange(of: appState.activeProjectID) { _, projectID in
            model.projectSwitchObservations += 1
            model.expansion.cancelAutoExpandRequest()
            guard autoExpandWorktrees,
                  projectID == project.id,
                  project.worktreesEnabled
            else { return }
            model.expansion.requestAutoExpand(projectID: project.id)
        }
    }
}
