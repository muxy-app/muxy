import Foundation
import Testing

@testable import Muxy

@Suite("AppState.restoreSelection")
@MainActor
struct AppStateRestoreTests {
    @Test("skipped project IDs do not restore their persisted tabs")
    func skippedProjectsAreNotRestored() {
        let project = Project(name: "api", path: "~/code/api")
        let worktree = Worktree(name: project.name, path: project.path, isPrimary: true)
        let snapshots = makeSnapshots(project: project, worktree: worktree)
        let appState = makeAppState(snapshots: snapshots)

        appState.restoreSelection(
            projects: [project],
            worktrees: [project.id: [worktree]],
            skippingProjectIDs: [project.id]
        )

        #expect(appState.workspaceRoot(for: project.id) == nil)
    }

    @Test("unskipped project IDs restore their persisted tabs")
    func unskippedProjectsAreRestored() {
        let project = Project(name: "api", path: "/tmp/api")
        let worktree = Worktree(name: project.name, path: project.path, isPrimary: true)
        let snapshots = makeSnapshots(project: project, worktree: worktree)
        let appState = makeAppState(snapshots: snapshots)

        appState.restoreSelection(
            projects: [project],
            worktrees: [project.id: [worktree]]
        )

        #expect(appState.workspaceRoot(for: project.id) != nil)
    }

    @Test("agent sessions persist only on exact changes and clear safely")
    func agentSessionPersistence() throws {
        let project = Project(name: "api", path: "/tmp/api")
        let worktree = Worktree(name: project.name, path: project.path, isPrimary: true)
        let snapshots = makeSnapshots(project: project, worktree: worktree)
        let persistence = WorkspacePersistenceStub(snapshots: snapshots)
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: persistence
        )
        appState.restoreSelection(
            projects: [project],
            worktrees: [project.id: [worktree]]
        )
        let pane = try #require(appState.workspaceRoot(for: project.id)?.allTabs().first?.content.pane)

        appState.updateAgentSession(paneID: pane.id, providerID: "opencode", sessionID: "ses_exact")
        appState.updateAgentSession(paneID: pane.id, providerID: "opencode", sessionID: "ses_exact")
        appState.clearAgentSession(paneID: pane.id, providerID: "claude", sessionID: "ses_exact")

        #expect(pane.agentSession == AgentSessionReference(providerID: "opencode", sessionID: "ses_exact"))
        #expect(persistence.saveCount == 1)

        appState.updateAgentSession(
            paneID: pane.id,
            providerID: "claude",
            sessionID: "late-session",
            replacesExisting: false
        )

        #expect(pane.agentSession == AgentSessionReference(providerID: "opencode", sessionID: "ses_exact"))
        #expect(persistence.saveCount == 1)

        appState.clearAgentSession(paneID: pane.id, providerID: "opencode", sessionID: "ses_exact")

        #expect(pane.agentSession == nil)
        #expect(persistence.saveCount == 2)
    }

    @Test("termination freezes agent session state before persistence")
    func terminationFreezesAgentSessionState() throws {
        let project = Project(name: "api", path: "/tmp/api")
        let worktree = Worktree(name: project.name, path: project.path, isPrimary: true)
        let persistence = WorkspacePersistenceStub(snapshots: makeSnapshots(project: project, worktree: worktree))
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: persistence
        )
        appState.restoreSelection(projects: [project], worktrees: [project.id: [worktree]])
        let pane = try #require(appState.workspaceRoot(for: project.id)?.allTabs().first?.content.pane)
        appState.updateAgentSession(paneID: pane.id, providerID: "claude", sessionID: "session-a")

        appState.freezeAgentSessionStateForTermination()
        appState.clearAgentSession(paneID: pane.id)
        appState.updateAgentSession(paneID: pane.id, providerID: "claude", sessionID: "session-b")

        #expect(pane.agentSession == AgentSessionReference(providerID: "claude", sessionID: "session-a"))
        #expect(persistence.saveCount == 1)
    }

    private func makeSnapshots(project: Project, worktree: Worktree) -> [WorkspaceSnapshot] {
        let key = WorktreeKey(projectID: project.id, worktreeID: worktree.id)
        let area = TabArea(projectPath: project.path)
        area.createTab()
        return WorkspaceRestorer.snapshotAll(
            workspaceRoots: [key: .tabArea(area)],
            focusedAreaID: [key: area.id]
        )
    }

    private func makeAppState(snapshots: [WorkspaceSnapshot]) -> AppState {
        AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub(snapshots: snapshots)
        )
    }
}

private final class WorkspacePersistenceStub: WorkspacePersisting {
    private var snapshots: [WorkspaceSnapshot]
    private(set) var saveCount = 0

    init(snapshots: [WorkspaceSnapshot] = []) {
        self.snapshots = snapshots
    }

    func loadWorkspaces() throws -> [WorkspaceSnapshot] { snapshots }
    func saveWorkspaces(_ workspaces: [WorkspaceSnapshot]) throws {
        snapshots = workspaces
        saveCount += 1
    }
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
    func removeView(for paneID: UUID) {}
    func needsConfirmQuit(for paneID: UUID) -> Bool { false }
}
