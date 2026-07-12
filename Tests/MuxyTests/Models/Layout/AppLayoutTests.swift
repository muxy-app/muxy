import Foundation
import Testing

@testable import Muxy

@Suite("AppLayout")
@MainActor
struct AppLayoutTests {
    @Test("project focused resolves the project focused provider")
    func projectFocusedProvider() {
        #expect(AppLayout.projectFocused.provider is ProjectFocusedLayout)
    }

    @Test("tab focused resolves the tab focused provider")
    func tabFocusedProvider() {
        #expect(AppLayout.tabFocused.provider is TabFocusedLayout)
    }

    @Test("default value is project focused")
    func defaultValueIsProjectFocused() {
        #expect(AppLayout.defaultValue == .projectFocused)
    }

    @Test("raw value round-trips through the initializer")
    func rawValueRoundTrips() {
        for layout in AppLayout.allCases {
            #expect(AppLayout(rawValue: layout.rawValue) == layout)
        }
    }

    @Test("tab focused sidebar keeps every project visible outside focus mode")
    func tabFocusedSidebarKeepsEveryProjectVisible() {
        let first = Project(name: "First", path: "/tmp/first")
        let second = Project(name: "Second", path: "/tmp/second")

        let projects = TabFocusedSidebarProjectSelection.resolve(
            projects: [first, second],
            focusMode: false,
            activeProjectID: first.id
        )

        #expect(projects == [first, second])
    }

    @Test("tab focused sidebar focus mode keeps only the active project")
    func tabFocusedSidebarFocusModeKeepsActiveProject() {
        let first = Project(name: "First", path: "/tmp/first")
        let second = Project(name: "Second", path: "/tmp/second")

        let projects = TabFocusedSidebarProjectSelection.resolve(
            projects: [first, second],
            focusMode: true,
            activeProjectID: second.id
        )

        #expect(projects == [second])
    }
}

@Suite("AppLayoutStore")
@MainActor
struct AppLayoutStoreTests {
    @Test("defaults to project focused when nothing is stored")
    func defaultsToProjectFocused() throws {
        let (defaults, name) = try makeDefaults()
        defer { defaults.removePersistentDomain(forName: name) }

        let store = AppLayoutStore(defaults: defaults)

        #expect(store.layout == .projectFocused)
    }

    @Test("restores the stored layout")
    func restoresStoredLayout() throws {
        let (defaults, name) = try makeDefaults()
        defer { defaults.removePersistentDomain(forName: name) }
        defaults.set(AppLayout.tabFocused.rawValue, forKey: AppLayout.storageKey)

        let store = AppLayoutStore(defaults: defaults)

        #expect(store.layout == .tabFocused)
    }

    @Test("set persists the new layout")
    func setPersistsLayout() throws {
        let (defaults, name) = try makeDefaults()
        defer { defaults.removePersistentDomain(forName: name) }
        let store = AppLayoutStore(defaults: defaults)

        store.set(.tabFocused)

        #expect(store.layout == .tabFocused)
        #expect(defaults.string(forKey: AppLayout.storageKey) == AppLayout.tabFocused.rawValue)
    }

    @Test("toggle alternates between the two layouts")
    func toggleAlternates() throws {
        let (defaults, name) = try makeDefaults()
        defer { defaults.removePersistentDomain(forName: name) }
        let store = AppLayoutStore(defaults: defaults)

        store.toggle()
        #expect(store.layout == .tabFocused)

        store.toggle()
        #expect(store.layout == .projectFocused)
    }

    private func makeDefaults() throws -> (UserDefaults, String) {
        let suiteName = "AppLayoutStoreTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            Issue.record("Unable to create isolated UserDefaults suite")
            throw AppLayoutTestError.unavailableDefaults
        }
        defaults.removePersistentDomain(forName: suiteName)
        return (defaults, suiteName)
    }
}

@Suite("TabFocusedSidebarState")
@MainActor
struct TabFocusedSidebarStateTests {
    @Test("expansion default is returned when nothing is stored")
    func expansionDefault() throws {
        let (defaults, name) = try makeDefaults()
        defer { defaults.removePersistentDomain(forName: name) }
        let state = TabFocusedSidebarState(defaults: defaults)
        let projectID = UUID()

        #expect(state.isExpanded(projectID, default: true))
        #expect(!state.isExpanded(projectID, default: false))
    }

    @Test("set persists and round-trips the expansion state")
    func setPersistsExpansion() throws {
        let (defaults, name) = try makeDefaults()
        defer { defaults.removePersistentDomain(forName: name) }
        let state = TabFocusedSidebarState(defaults: defaults)
        let projectID = UUID()

        state.set(projectID, expanded: true)

        #expect(state.isExpanded(projectID, default: false))
        #expect(state.isExpandedPersisted(projectID))

        let reloaded = TabFocusedSidebarState(defaults: defaults)
        #expect(reloaded.isExpandedPersisted(projectID))
    }

    private func makeDefaults() throws -> (UserDefaults, String) {
        let suiteName = "TabFocusedSidebarStateTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            Issue.record("Unable to create isolated UserDefaults suite")
            throw AppLayoutTestError.unavailableDefaults
        }
        defaults.removePersistentDomain(forName: suiteName)
        return (defaults, suiteName)
    }
}

private enum AppLayoutTestError: Error {
    case unavailableDefaults
}
