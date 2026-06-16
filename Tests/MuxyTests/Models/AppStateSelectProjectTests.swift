import Foundation
import Testing

@testable import Muxy

@Suite("AppState.selectProject")
@MainActor
struct AppStateSelectProjectTests {
    @Test("selecting a new project notifies onProjectSelected")
    func notifiesOnNewSelection() {
        let project = Project(name: "api", path: "/tmp/api")
        let worktree = Worktree(name: project.name, path: project.path, isPrimary: true)
        let appState = makeAppState()
        var selected: [UUID] = []
        appState.onProjectSelected = { selected.append($0) }

        appState.selectProject(project, worktree: worktree)

        #expect(selected == [project.id])
    }

    @Test("reselecting the active project does not notify again")
    func skipsNotificationWhenAlreadyActive() {
        let project = Project(name: "api", path: "/tmp/api")
        let worktree = Worktree(name: project.name, path: project.path, isPrimary: true)
        let appState = makeAppState()
        var selected: [UUID] = []
        appState.onProjectSelected = { selected.append($0) }

        appState.selectProject(project, worktree: worktree)
        appState.selectProject(project, worktree: worktree)

        #expect(selected == [project.id])
    }

    private func makeAppState() -> AppState {
        AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
    }
}

private final class WorkspacePersistenceStub: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_ workspaces: [WorkspaceSnapshot]) throws {}
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
