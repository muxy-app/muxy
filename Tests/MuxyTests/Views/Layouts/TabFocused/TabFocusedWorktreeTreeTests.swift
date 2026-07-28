import Foundation
import Testing

@testable import Muxy

@Suite("TabFocused worktree sidebar state")
@MainActor
struct TabFocusedWorktreeSidebarStateTests {
    private func makeDefaults() -> (UserDefaults, String) {
        let suiteName = "muxy.tests.tabFocusedSidebar.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suiteName)!
        return (defaults, suiteName)
    }

    @Test("isExpanded falls back to supplied default when no value is stored")
    func isExpandedDefault() {
        let (defaults, suiteName) = makeDefaults()
        defer { defaults.removePersistentDomain(forName: suiteName) }

        let state = TabFocusedSidebarState(defaults: defaults)
        let id = UUID()

        #expect(state.isExpanded(id, default: true))
        #expect(!state.isExpanded(id, default: false))
    }

    @Test("set stores expansion value and overrides the default")
    func setOverridesDefault() {
        let (defaults, suiteName) = makeDefaults()
        defer { defaults.removePersistentDomain(forName: suiteName) }

        let state = TabFocusedSidebarState(defaults: defaults)
        let id = UUID()

        state.set(id, expanded: true)

        #expect(state.isExpanded(id, default: false))
        #expect(state.isExpandedPersisted(id))
    }

    @Test("set false persists collapsed state")
    func setFalsePersists() {
        let (defaults, suiteName) = makeDefaults()
        defer { defaults.removePersistentDomain(forName: suiteName) }

        let state = TabFocusedSidebarState(defaults: defaults)
        let id = UUID()

        state.set(id, expanded: true)
        state.set(id, expanded: false)

        #expect(!state.isExpanded(id, default: true))
        #expect(!state.isExpandedPersisted(id))
    }

    @Test("expansion state is stored per row id")
    func stateIsPerRow() {
        let (defaults, suiteName) = makeDefaults()
        defer { defaults.removePersistentDomain(forName: suiteName) }

        let state = TabFocusedSidebarState(defaults: defaults)
        let firstID = UUID()
        let secondID = UUID()

        state.set(firstID, expanded: true)
        state.set(secondID, expanded: false)

        #expect(state.isExpanded(firstID, default: false))
        #expect(!state.isExpanded(secondID, default: true))
    }

    @Test("focus mode is disabled by default and persists")
    func focusModePersists() {
        let (defaults, suiteName) = makeDefaults()
        defer { defaults.removePersistentDomain(forName: suiteName) }

        let first = TabFocusedSidebarState(defaults: defaults)
        #expect(!first.focusMode)

        first.focusMode = true

        let second = TabFocusedSidebarState(defaults: defaults)
        #expect(second.focusMode)
    }
}

@Suite("Worktree sidebar display")
struct WorktreeSidebarDisplayTests {
    @Test("primary worktree displays Main Worktree")
    func primaryDisplayName() {
        let worktree = Worktree(name: "main", path: "/projects/app", isPrimary: true)

        #expect(worktree.sidebarDisplayName == "Main Worktree")
    }

    @Test("non-primary worktree displays its name")
    func nonPrimaryDisplayName() {
        let worktree = Worktree(name: "feature-x", path: "/projects/app-feature-x", isPrimary: false)

        #expect(worktree.sidebarDisplayName == "feature-x")
    }
}

@Suite("TabFocusedSidebarProjectSelection")
struct TabFocusedSidebarProjectSelectionTests {
    @Test("returns all projects when focus mode is off")
    func focusModeOff() {
        let a = Project(name: "a", path: "/a")
        let b = Project(name: "b", path: "/b")

        let result = TabFocusedSidebarProjectSelection.resolve(
            projects: [a, b],
            focusMode: false,
            activeProjectID: a.id
        )

        #expect(result == [a, b])
    }

    @Test("returns active project only when focus mode is on")
    func focusModeOn() {
        let a = Project(name: "a", path: "/a")
        let b = Project(name: "b", path: "/b")

        let result = TabFocusedSidebarProjectSelection.resolve(
            projects: [a, b],
            focusMode: true,
            activeProjectID: b.id
        )

        #expect(result == [b])
    }

    @Test("returns all projects when focus mode on but no active project matches")
    func focusModeOnNoMatch() {
        let a = Project(name: "a", path: "/a")
        let b = Project(name: "b", path: "/b")
        let other = UUID()

        let result = TabFocusedSidebarProjectSelection.resolve(
            projects: [a, b],
            focusMode: true,
            activeProjectID: other
        )

        #expect(result == [a, b])
    }

    @Test("returns all projects when active project id is nil")
    func nilActiveProject() {
        let a = Project(name: "a", path: "/a")

        let result = TabFocusedSidebarProjectSelection.resolve(
            projects: [a],
            focusMode: true,
            activeProjectID: nil
        )

        #expect(result == [a])
    }
}

@Suite("Focused sidebar rows")
struct TabFocusedSidebarRowsTests {
    @Test("grouped focused layouts contain project rows only")
    func groupedRows() {
        var project = Project(name: "Repo", path: "/repo")
        project.worktreesEnabled = true
        let secondary = Worktree(name: "feature", path: "/feature", isPrimary: false)

        for content in [TabFocusedSidebarContent.tabs, .agents] {
            let rows = TabFocusedSidebarRows.resolve(
                projects: [project],
                content: content,
                groupWorktrees: true,
                worktreesForProject: { _ in [secondary] },
                hasTabs: { _ in true }
            )

            #expect(rows.map(\.id) == [project.id])
        }
    }

    @Test("ungrouped tab focused includes only worktrees with tabs")
    func ungroupedTabRows() {
        var project = Project(name: "Repo", path: "/repo")
        project.worktreesEnabled = true
        let primary = Worktree(name: "Repo", path: "/repo", isPrimary: true)
        let open = Worktree(name: "open", path: "/open", isPrimary: false)
        let closed = Worktree(name: "closed", path: "/closed", isPrimary: false)

        let rows = TabFocusedSidebarRows.resolve(
            projects: [project],
            content: .tabs,
            groupWorktrees: false,
            worktreesForProject: { _ in [primary, open, closed] },
            hasTabs: { $0.worktreeID == open.id }
        )

        #expect(rows.map(\.id) == [project.id, open.id])
    }

    @Test("ungrouped agents focused includes every secondary worktree")
    func ungroupedAgentRows() {
        var project = Project(name: "Repo", path: "/repo")
        project.worktreesEnabled = true
        let primary = Worktree(name: "Repo", path: "/repo", isPrimary: true)
        let first = Worktree(name: "first", path: "/first", isPrimary: false)
        let second = Worktree(name: "second", path: "/second", isPrimary: false)

        let rows = TabFocusedSidebarRows.resolve(
            projects: [project],
            content: .agents,
            groupWorktrees: false,
            worktreesForProject: { _ in [primary, first, second] },
            hasTabs: { _ in false }
        )

        #expect(rows.map(\.id) == [project.id, first.id, second.id])
    }
}
