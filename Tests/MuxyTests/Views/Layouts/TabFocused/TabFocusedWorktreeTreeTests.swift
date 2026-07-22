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
