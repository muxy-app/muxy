import Foundation
import Testing

@testable import Muxy

@Suite("SocketCommandHandler")
@MainActor
struct SocketCommandHandlerTests {
    private let testPath = "/tmp/test"

    @Test("unknown command returns error")
    func unknownCommand() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("bogus", appState: appState)
        #expect(result.hasPrefix("error:"))
    }

    @Test("split-right returns new pane ID")
    func splitReturnsNewPaneID() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("split-right", appState: appState)
        #expect(!result.hasPrefix("error:"))
        #expect(UUID(uuidString: result) != nil)
    }

    @Test("split-down returns new pane ID")
    func splitDownReturnsNewPaneID() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("split-down", appState: appState)
        #expect(!result.hasPrefix("error:"))
        #expect(UUID(uuidString: result) != nil)
    }

    @Test("split-right with command returns new pane ID")
    func splitWithCommandReturnsNewPaneID() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("split-right||echo hello", appState: appState)
        #expect(!result.hasPrefix("error:"))
        #expect(UUID(uuidString: result) != nil)
    }

    @Test("split-right preserves commands containing pipes")
    func splitPreservesCommandPipes() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("split-right||echo a | wc", appState: appState)
        let paneID = UUID(uuidString: result)
        #expect(paneID != nil)
        #expect(paneID.flatMap { pane(with: $0, appState: appState)?.startupCommand } == "(echo a | wc); exec \"$0\" -l")
    }

    @Test("split fails without active project")
    func splitFailsWithoutActiveProject() async {
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let result = await SocketCommandHandler.handleRequest("split-right", appState: appState)
        #expect(result == "error:no active project")
    }

    @Test("send fails with missing args")
    func sendFailsMissingArgs() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("send|\(UUID().uuidString)", appState: appState)
        #expect(result.hasPrefix("error:"))
    }

    @Test("send-keys fails with unsupported key")
    func sendKeysFailsUnsupportedKey() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("send-keys|\(UUID().uuidString)|F13", appState: appState)
        #expect(result.hasPrefix("error:"))
    }

    @Test("send-keys fails with missing key")
    func sendKeysFailsMissingKey() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("send-keys|\(UUID().uuidString)", appState: appState)
        #expect(result.hasPrefix("error:"))
    }

    @Test("close-pane fails with nonexistent pane")
    func closePaneFailsNonexistentPane() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("close-pane|\(UUID().uuidString)", appState: appState)
        #expect(result.hasPrefix("error:pane not found"))
    }

    @Test("close-pane fails with invalid pane ID")
    func closePaneFailsInvalidPaneID() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("close-pane|not-a-uuid", appState: appState)
        #expect(result == "error:invalid pane ID")
    }

    @Test("rename-pane fails with nonexistent pane")
    func renamePaneFailsNonexistentPane() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("rename-pane|\(UUID().uuidString)|Test", appState: appState)
        #expect(result.hasPrefix("error:pane not found"))
    }

    @Test("rename-pane fails with missing title")
    func renamePaneFailsMissingTitle() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("rename-pane|\(UUID().uuidString)", appState: appState)
        #expect(result.hasPrefix("error:"))
    }

    @Test("list-panes returns empty when no panes")
    func listPanesEmpty() async {
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let result = await SocketCommandHandler.handleRequest("list-panes", appState: appState)
        #expect(result.isEmpty)
    }

    @Test("list-panes returns tab-separated pane info")
    func listPanesReturnsPanes() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("list-panes", appState: appState)
        #expect(!result.isEmpty)
        let fields = result.components(separatedBy: "\t")
        #expect(fields.count >= 4)
        #expect(UUID(uuidString: fields[0]) != nil)
    }

    @Test("split-right with from pane targets correct area")
    func splitWithFromPane() async {
        let appState = makeAppState()
        let firstPaneID = appState.workspaceRoots.values.first!.allAreas().first!.tabs.first!.content.pane!.id
        let result = await SocketCommandHandler.handleRequest("split-right|" + firstPaneID.uuidString + "|", appState: appState)
        #expect(!result.hasPrefix("error:"))
        #expect(UUID(uuidString: result) != nil)
    }

    @Test("list-projects returns projects with active marker")
    func listProjects() async {
        let project = Project(name: "Test Project", path: testPath)
        let worktree = Worktree(name: project.name, path: project.path, isPrimary: true)
        let appState = makeAppState(projectID: project.id, worktreeID: worktree.id)
        let stores = makeStores(project: project, worktree: worktree)

        let result = await SocketCommandHandler.handleRequest(
            "list-projects",
            appState: appState,
            projectStore: stores.projectStore,
            worktreeStore: stores.worktreeStore
        )

        #expect(result.contains(project.id.uuidString))
        #expect(result.contains("Test Project"))
        #expect(result.contains("\ttrue"))
    }

    @Test("switch-project selects matching project")
    func switchProject() async {
        let first = Project(name: "First", path: "/tmp/first")
        let second = Project(name: "Second", path: "/tmp/second")
        let firstWorktree = Worktree(name: first.name, path: first.path, isPrimary: true)
        let secondWorktree = Worktree(name: second.name, path: second.path, isPrimary: true)
        let appState = makeAppState(projectID: first.id, worktreeID: firstWorktree.id)
        let stores = makeStores(projects: [first, second], worktrees: [first.id: [firstWorktree], second.id: [secondWorktree]])

        let result = await SocketCommandHandler.handleRequest(
            "switch-project|Second",
            appState: appState,
            projectStore: stores.projectStore,
            worktreeStore: stores.worktreeStore
        )

        #expect(result == "ok")
        #expect(appState.activeProjectID == second.id)
        #expect(appState.activeWorktreeID[second.id] == secondWorktree.id)
    }

    @Test("switch-worktree selects matching worktree")
    func switchWorktree() async {
        let project = Project(name: "Test Project", path: testPath)
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let feature = Worktree(name: "Feature", path: "/tmp/test-feature", branch: "feature", isPrimary: false)
        let appState = makeAppState(projectID: project.id, worktreeID: primary.id)
        let stores = makeStores(projects: [project], worktrees: [project.id: [primary, feature]])

        let result = await SocketCommandHandler.handleRequest(
            "switch-worktree|feature",
            appState: appState,
            projectStore: stores.projectStore,
            worktreeStore: stores.worktreeStore
        )

        #expect(result == "ok")
        #expect(appState.activeWorktreeID[project.id] == feature.id)
    }

    @Test("create-worktree creates and selects worktree")
    func createWorktree() async throws {
        let project = Project(name: "Test Project", path: testPath)
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let appState = makeAppState(projectID: project.id, worktreeID: primary.id)
        let createdPath = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-test-worktree-")
            .appendingPathComponent(UUID().uuidString)
            .path
        let stores = makeStores(
            projects: [project],
            worktrees: [project.id: [primary]],
            addGitWorktree: { _, _, _, _, _ in }
        )

        let result = await SocketCommandHandler.handleRequest(
            "create-worktree|Feature|feature|\(project.name)|\(createdPath)|true|main",
            appState: appState,
            projectStore: stores.projectStore,
            worktreeStore: stores.worktreeStore
        )

        let fields = result.components(separatedBy: "\t")
        #expect(fields.first == "ok")
        #expect(fields.count == 5)
        let worktree = try #require(stores.worktreeStore.list(for: project.id).first { $0.name == "Feature" })
        #expect(worktree.branch == "feature")
        #expect(worktree.path == createdPath)
        #expect(worktree.ownsBranch)
        #expect(appState.activeWorktreeID[project.id] == worktree.id)
    }

    @Test("list-tabs includes active tab")
    func listTabs() async {
        let appState = makeAppState()
        appState.dispatch(.createTab(projectID: appState.activeProjectID!, areaID: nil))

        let result = await SocketCommandHandler.handleRequest("list-tabs", appState: appState)

        #expect(result.contains("\tterminal\t"))
        #expect(result.contains("\ttrue"))
    }

    @Test("switch-tab selects tab by index")
    func switchTabByIndex() async {
        let appState = makeAppState()
        let projectID = appState.activeProjectID!
        appState.dispatch(.createTab(projectID: projectID, areaID: nil))
        let area = appState.focusedArea(for: projectID)!
        let firstTabID = area.tabs[0].id

        let result = await SocketCommandHandler.handleRequest("switch-tab|0", appState: appState)

        #expect(result == "ok")
        #expect(area.activeTabID == firstTabID)
    }

    @Test("switch-tab reports invalid index")
    func switchTabByInvalidIndex() async {
        let appState = makeAppState()

        let result = await SocketCommandHandler.handleRequest("switch-tab|99", appState: appState)

        #expect(result.hasPrefix("error:tab not found"))
    }

    @Test("new-tab creates terminal tab")
    func newTab() async {
        let appState = makeAppState()
        let projectID = appState.activeProjectID!
        let before = appState.focusedArea(for: projectID)!.tabs.count

        let result = await SocketCommandHandler.handleRequest("new-tab", appState: appState)

        #expect(UUID(uuidString: result) != nil)
        #expect(appState.focusedArea(for: projectID)!.tabs.count == before + 1)
    }

    private func pane(with paneID: UUID, appState: AppState) -> TerminalPaneState? {
        for root in appState.workspaceRoots.values {
            for area in root.allAreas() {
                for tab in area.tabs where tab.content.pane?.id == paneID {
                    return tab.content.pane
                }
            }
        }
        return nil
    }

    private func makeAppState(
        projectID: UUID = UUID(),
        worktreeID: UUID = UUID()
    ) -> AppState {
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = TabArea(projectPath: testPath)
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = .tabArea(area)
        appState.focusedAreaID[key] = area.id
        return appState
    }

    private func makeStores(
        project: Project,
        worktree: Worktree
    ) -> (projectStore: ProjectStore, worktreeStore: WorktreeStore) {
        makeStores(projects: [project], worktrees: [project.id: [worktree]])
    }

    private func makeStores(
        projects: [Project],
        worktrees: [UUID: [Worktree]],
        addGitWorktree: @escaping @Sendable (String, String, String, Bool, String?) async throws -> Void = { _, _, _, _, _ in }
    ) -> (projectStore: ProjectStore, worktreeStore: WorktreeStore) {
        let projectStore = ProjectStore(persistence: ProjectPersistenceSocketStub(projects: projects))
        let worktreeStore = WorktreeStore(
            persistence: WorktreePersistenceSocketStub(worktrees: worktrees),
            addGitWorktree: addGitWorktree,
            projects: projects
        )
        return (projectStore, worktreeStore)
    }
}

private final class WorkspacePersistenceStub: WorkspacePersisting {
    private var snapshots: [WorkspaceSnapshot] = []
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { snapshots }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
}

private final class ProjectPersistenceSocketStub: ProjectPersisting {
    private var projects: [Project]

    init(projects: [Project]) {
        self.projects = projects
    }

    func loadProjects() throws -> [Project] { projects }
    func saveProjects(_ projects: [Project]) throws { self.projects = projects }
}

private final class WorktreePersistenceSocketStub: WorktreePersisting {
    private var worktrees: [UUID: [Worktree]]

    init(worktrees: [UUID: [Worktree]]) {
        self.worktrees = worktrees
    }

    func loadWorktrees(projectID: UUID) throws -> [Worktree] { worktrees[projectID] ?? [] }
    func saveWorktrees(_ worktrees: [Worktree], projectID: UUID) throws { self.worktrees[projectID] = worktrees }
    func removeWorktrees(projectID: UUID) throws { worktrees[projectID] = nil }
}

@MainActor
private final class SelectionStoreStub: ActiveProjectSelectionStoring {
    private var activeProjectID: UUID?
    private var activeWorktreeIDs: [UUID: UUID] = [:]
    func loadActiveProjectID() -> UUID? { activeProjectID }
    func saveActiveProjectID(_ id: UUID?) { activeProjectID = id }
    func loadActiveWorktreeIDs() -> [UUID: UUID] { activeWorktreeIDs }
    func saveActiveWorktreeIDs(_ ids: [UUID: UUID]) { activeWorktreeIDs = ids }
}

@MainActor
private final class TerminalViewRemovingStub: TerminalViewRemoving {
    func removeView(for _: UUID) {}
    func needsConfirmQuit(for _: UUID) -> Bool { false }
}
