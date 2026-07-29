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

    @Test("agents focused resolves the agents focused provider")
    func agentsFocusedProvider() {
        #expect(AppLayout.agentsFocused.provider is AgentsFocusedLayout)
    }

    @Test("project focused keeps tabs in the title bar")
    func projectFocusedTitleBar() {
        #expect(ProjectFocusedLayout().topbar == .tabStrip)
    }

    @Test("tab focused uses the project title")
    func tabFocusedTitleBar() {
        #expect(TabFocusedLayout().topbar == .projectTitle)
    }

    @Test("agents focused keeps tabs in the title bar")
    func agentsFocusedTitleBar() {
        #expect(AgentsFocusedLayout().topbar == .tabStrip)
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

    @Test("agents focused sidebar ignores tab focused focus mode")
    func agentsFocusedSidebarKeepsEveryProjectVisible() {
        let first = Project(name: "First", path: "/tmp/first")
        let second = Project(name: "Second", path: "/tmp/second")

        let projects = TabFocusedSidebarProjectSelection.resolve(
            projects: [first, second],
            focusMode: true,
            activeProjectID: second.id,
            content: .agents
        )

        #expect(projects == [first, second])
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

    @Test("toggle cycles through every layout")
    func toggleCycles() throws {
        let (defaults, name) = try makeDefaults()
        defer { defaults.removePersistentDomain(forName: name) }
        let store = AppLayoutStore(defaults: defaults)

        store.toggle()
        #expect(store.layout == .tabFocused)

        store.toggle()
        #expect(store.layout == .agentsFocused)

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

@Suite("AgentsFocusedTabSelection")
@MainActor
struct AgentsFocusedTabSelectionTests {
    @Test("includes detected top-level and child agent tabs in parent order")
    func includesAgentTabs() {
        let rootArea = TabArea(projectPath: "/tmp/project")
        let firstRoot = rootArea.tabs[0]
        let secondRoot = TerminalTab(pane: TerminalPaneState(projectPath: "/tmp/project"))
        rootArea.insertExistingTab(secondRoot)
        let child = TerminalTab(
            pane: TerminalPaneState(projectPath: "/tmp/project"),
            parentTabID: firstRoot.id
        )
        let childArea = TabArea(projectPath: "/tmp/project", existingTab: child)
        let root = SplitNode.split(SplitBranch(
            direction: .horizontal,
            first: .tabArea(rootArea),
            second: .tabArea(childArea)
        ))
        let agentPaneIDs = Set([
            firstRoot.content.pane?.id,
            child.content.pane?.id,
        ].compactMap(\.self))

        let locations = AgentsFocusedTabSelection.resolve(
            root: root,
            topLevelTabs: [
                (area: rootArea, tab: firstRoot),
                (area: rootArea, tab: secondRoot),
            ],
            providerID: { agentPaneIDs.contains($0) ? "codex" : nil }
        )

        #expect(locations.map(\.tab.id) == [firstRoot.id, child.id])
        #expect(locations.map(\.area.id) == [rootArea.id, childArea.id])
    }

    @Test("keeps child agent tabs when their parent is not an agent")
    func includesChildWithoutAgentParent() {
        let rootArea = TabArea(projectPath: "/tmp/project")
        let parent = rootArea.tabs[0]
        let child = TerminalTab(
            pane: TerminalPaneState(projectPath: "/tmp/project"),
            parentTabID: parent.id
        )
        let childArea = TabArea(projectPath: "/tmp/project", existingTab: child)
        let root = SplitNode.split(SplitBranch(
            direction: .horizontal,
            first: .tabArea(rootArea),
            second: .tabArea(childArea)
        ))

        let locations = AgentsFocusedTabSelection.resolve(
            root: root,
            topLevelTabs: [(area: rootArea, tab: parent)],
            providerID: { $0 == child.content.pane?.id ? "claude" : nil }
        )

        #expect(locations.map(\.tab.id) == [child.id])
        #expect(locations.first?.area.id == childArea.id)
    }
}

@Suite("AgentsFocusedTabLauncher")
@MainActor
struct AgentsFocusedTabLauncherTests {
    @Test("launching into an inactive worktree creates its workspace and agent tab")
    func launchesIntoInactiveWorktreeWithoutWorkspace() throws {
        let activeProject = Project(name: "Active", path: "/tmp/active")
        let targetProject = Project(name: "Target", path: "/tmp/target")
        let activeWorktree = Worktree(name: "Active", path: activeProject.path, isPrimary: true)
        let targetPrimary = Worktree(name: "Target", path: targetProject.path, isPrimary: true)
        let targetWorktree = Worktree(name: "Agent", path: "/tmp/target-agent", isPrimary: false)
        let worktreeStore = WorktreeStore(
            persistence: AgentLaunchWorktreePersistenceStub(storage: [
                activeProject.id: [activeWorktree],
                targetProject.id: [targetPrimary, targetWorktree],
            ]),
            projects: [activeProject, targetProject]
        )
        let appState = AppState(
            selectionStore: AgentLaunchSelectionStoreStub(),
            terminalViews: AgentLaunchTerminalViewRemovingStub(),
            workspacePersistence: AgentLaunchWorkspacePersistenceStub()
        )
        appState.selectProject(activeProject, worktree: activeWorktree)
        let targetKey = WorktreeKey(projectID: targetProject.id, worktreeID: targetWorktree.id)
        #expect(appState.workspaceRoots[targetKey] == nil)

        AgentsFocusedTabLauncher.launch(
            request: AgentsFocusedTabLaunchRequest(
                project: targetProject,
                worktree: targetWorktree,
                providerID: "codex",
                name: "Codex",
                command: "codex"
            ),
            appState: appState,
            worktreeStore: worktreeStore
        )

        let root = try #require(appState.workspaceRoots[targetKey])
        let agentTabs = root.allAreas().flatMap(\.tabs).filter {
            $0.content.pane?.startupCommand == "codex"
        }
        #expect(appState.activeProjectID == targetProject.id)
        #expect(appState.activeWorktreeID[targetProject.id] == targetWorktree.id)
        #expect(agentTabs.count == 1)
        #expect(agentTabs.first?.content.pane?.title == "Codex")
        let paneID = try #require(agentTabs.first?.content.pane?.id)
        defer { DetectedAgentStore.shared.resetPane(paneID) }
        #expect(DetectedAgentStore.shared.agent(for: paneID) == "codex")
        let otherPaneIDs = root.allAreas()
            .flatMap(\.tabs)
            .compactMap(\.content.pane?.id)
            .filter { $0 != paneID }
        #expect(otherPaneIDs.allSatisfy { DetectedAgentStore.shared.agent(for: $0) == nil })
    }

    @Test("failed tab creation does not register an agent")
    func failedCreationDoesNotRegisterAgent() {
        let project = Project(name: "Target", path: "/tmp/target")
        let worktree = Worktree(name: "Target", path: project.path, isPrimary: true)
        let worktreeStore = WorktreeStore(
            persistence: AgentLaunchWorktreePersistenceStub(storage: [project.id: [worktree]]),
            projects: [project]
        )
        let appState = AppState(
            selectionStore: AgentLaunchSelectionStoreStub(),
            terminalViews: AgentLaunchTerminalViewRemovingStub(),
            workspacePersistence: AgentLaunchWorkspacePersistenceStub()
        )
        appState.selectProject(project, worktree: worktree)
        let agentsBeforeLaunch = DetectedAgentStore.shared.agents

        AgentsFocusedTabLauncher.launch(
            request: AgentsFocusedTabLaunchRequest(
                project: project,
                worktree: worktree,
                providerID: "codex",
                name: "Codex",
                command: " "
            ),
            appState: appState,
            worktreeStore: worktreeStore
        )

        #expect(DetectedAgentStore.shared.agents == agentsBeforeLaunch)
    }

    @Test("local agent launches appear before process detection")
    func localLaunchAppearsBeforeDetection() throws {
        let project = Project(name: "Target", path: "/tmp/target")
        let worktree = Worktree(name: "Target", path: project.path, isPrimary: true)
        let worktreeStore = WorktreeStore(
            persistence: AgentLaunchWorktreePersistenceStub(storage: [project.id: [worktree]]),
            projects: [project]
        )
        let appState = AppState(
            selectionStore: AgentLaunchSelectionStoreStub(),
            terminalViews: AgentLaunchTerminalViewRemovingStub(),
            workspacePersistence: AgentLaunchWorkspacePersistenceStub()
        )
        appState.selectProject(project, worktree: worktree)

        AgentsFocusedTabLauncher.launch(
            request: AgentsFocusedTabLaunchRequest(
                project: project,
                worktree: worktree,
                providerID: "codex",
                name: "Codex",
                command: "codex"
            ),
            appState: appState,
            worktreeStore: worktreeStore
        )

        let key = WorktreeKey(projectID: project.id, worktreeID: worktree.id)
        let paneID = try #require(appState.workspaceRoots[key]?.allTabs().last?.content.pane?.id)
        defer { DetectedAgentStore.shared.resetPane(paneID) }
        #expect(DetectedAgentStore.shared.agent(for: paneID) == "codex")
    }
}

@Suite("TabFocusedSidebarRowTap")
struct TabFocusedSidebarRowTapTests {
    @Test("tab focused rows only toggle expansion")
    func tabFocusedRowsToggle() {
        #expect(TabFocusedSidebarRowTap.resolve(content: .tabs, isActive: false) == .toggleExpansion)
        #expect(TabFocusedSidebarRowTap.resolve(content: .tabs, isActive: true) == .toggleExpansion)
    }

    @Test("agents focused rows activate when inactive")
    func agentsFocusedInactiveRowActivates() {
        #expect(TabFocusedSidebarRowTap.resolve(content: .agents, isActive: false) == .activateRow)
    }

    @Test("agents focused rows toggle expansion when already active")
    func agentsFocusedActiveRowToggles() {
        #expect(TabFocusedSidebarRowTap.resolve(content: .agents, isActive: true) == .toggleExpansion)
    }
}

@Suite("TabFocusedSidebarTarget")
@MainActor
struct TabFocusedSidebarTargetTests {
    @Test("activating another project selects it with its primary worktree")
    func activatesOtherProject() {
        let activeProject = Project(name: "Active", path: "/tmp/active")
        let targetProject = Project(name: "Target", path: "/tmp/target")
        let activeWorktree = Worktree(name: "Active", path: activeProject.path, isPrimary: true)
        let targetPrimary = Worktree(name: "Target", path: targetProject.path, isPrimary: true)
        let worktreeStore = WorktreeStore(
            persistence: AgentLaunchWorktreePersistenceStub(storage: [
                activeProject.id: [activeWorktree],
                targetProject.id: [targetPrimary],
            ]),
            projects: [activeProject, targetProject]
        )
        let appState = makeAppState()
        appState.selectProject(activeProject, worktree: activeWorktree)

        TabFocusedSidebarTarget.activate(
            project: targetProject,
            worktree: targetPrimary,
            appState: appState,
            worktreeStore: worktreeStore
        )

        #expect(appState.activeProjectID == targetProject.id)
        #expect(appState.activeWorktreeID[targetProject.id] == targetPrimary.id)
    }

    @Test("activating the primary row switches back from an active worktree")
    func activatesPrimaryWorktreeRow() {
        let project = Project(name: "Target", path: "/tmp/target")
        let primary = Worktree(name: "Target", path: project.path, isPrimary: true)
        let secondary = Worktree(name: "Agent", path: "/tmp/target-agent", isPrimary: false)
        let worktreeStore = WorktreeStore(
            persistence: AgentLaunchWorktreePersistenceStub(storage: [project.id: [primary, secondary]]),
            projects: [project]
        )
        let appState = makeAppState()
        appState.selectProject(project, worktree: secondary)

        TabFocusedSidebarTarget.activate(
            project: project,
            worktree: primary,
            appState: appState,
            worktreeStore: worktreeStore
        )

        #expect(appState.activeWorktreeID[project.id] == primary.id)
    }

    private func makeAppState() -> AppState {
        AppState(
            selectionStore: AgentLaunchSelectionStoreStub(),
            terminalViews: AgentLaunchTerminalViewRemovingStub(),
            workspacePersistence: AgentLaunchWorkspacePersistenceStub()
        )
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

private final class AgentLaunchWorkspacePersistenceStub: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
}

@MainActor
private final class AgentLaunchSelectionStoreStub: ActiveProjectSelectionStoring {
    func loadActiveProjectID() -> UUID? { nil }
    func saveActiveProjectID(_: UUID?) {}
    func loadActiveWorktreeIDs() -> [UUID: UUID] { [:] }
    func saveActiveWorktreeIDs(_: [UUID: UUID]) {}
}

@MainActor
private final class AgentLaunchTerminalViewRemovingStub: TerminalViewRemoving {
    func removeView(for _: UUID) {}
    func needsConfirmQuit(for _: UUID) -> Bool { false }
}

private final class AgentLaunchWorktreePersistenceStub: WorktreePersisting {
    private var storage: [UUID: [Worktree]]

    init(storage: [UUID: [Worktree]]) {
        self.storage = storage
    }

    func loadWorktrees(projectID: UUID) throws -> [Worktree] {
        storage[projectID] ?? []
    }

    func saveWorktrees(_ worktrees: [Worktree], projectID: UUID) throws {
        storage[projectID] = worktrees
    }

    func removeWorktrees(projectID: UUID) throws {
        storage.removeValue(forKey: projectID)
    }
}
