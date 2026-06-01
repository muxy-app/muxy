import Foundation
import Testing

@testable import Muxy

@Suite("TerminalViewMaterializer")
@MainActor
struct TerminalViewMaterializerTests {
    @Test("returns nil when pane id has no matching tab in any worktree")
    func returnsNilForUnknownPane() {
        let appState = makeAppState()
        let orphanPaneID = UUID()

        let result = TerminalViewMaterializer.ensureMaterialized(
            paneID: orphanPaneID,
            appState: appState
        )

        #expect(result == nil)
    }

    @Test("materializes a headless view when pane exists in a background worktree")
    func materializesHeadlessViewForBackgroundWorktree() {
        let projectID = UUID()
        let activeWorktreeID = UUID()
        let backgroundWorktreeID = UUID()
        let appState = makeAppStateWithWorktrees(
            projectID: projectID,
            activeWorktreeID: activeWorktreeID,
            backgroundWorktreeID: backgroundWorktreeID
        )
        let backgroundKey = WorktreeKey(projectID: projectID, worktreeID: backgroundWorktreeID)
        let backgroundPaneID = appState.workspaceRoots[backgroundKey]!
            .allAreas()[0]
            .tabs[0]
            .content
            .pane!
            .id

        #expect(TerminalViewRegistry.shared.existingView(for: backgroundPaneID) == nil)

        let result = TerminalViewMaterializer.ensureMaterialized(
            paneID: backgroundPaneID,
            appState: appState
        )

        #expect(result != nil)
        #expect(TerminalViewRegistry.shared.existingView(for: backgroundPaneID) === result)
    }

    @Test("returns the existing view when one is already registered")
    func returnsExistingViewWithoutRecreating() {
        let projectID = UUID()
        let activeWorktreeID = UUID()
        let backgroundWorktreeID = UUID()
        let appState = makeAppStateWithWorktrees(
            projectID: projectID,
            activeWorktreeID: activeWorktreeID,
            backgroundWorktreeID: backgroundWorktreeID
        )
        let backgroundKey = WorktreeKey(projectID: projectID, worktreeID: backgroundWorktreeID)
        let backgroundPaneID = appState.workspaceRoots[backgroundKey]!
            .allAreas()[0]
            .tabs[0]
            .content
            .pane!
            .id
        let first = TerminalViewMaterializer.ensureMaterialized(
            paneID: backgroundPaneID,
            appState: appState
        )
        let second = TerminalViewMaterializer.ensureMaterialized(
            paneID: backgroundPaneID,
            appState: appState
        )

        #expect(first === second)
    }

    @Test("populates worktree env vars on first materialization")
    func populatesWorktreeEnvVars() {
        let projectID = UUID()
        let activeWorktreeID = UUID()
        let backgroundWorktreeID = UUID()
        let appState = makeAppStateWithWorktrees(
            projectID: projectID,
            activeWorktreeID: activeWorktreeID,
            backgroundWorktreeID: backgroundWorktreeID
        )
        let backgroundKey = WorktreeKey(projectID: projectID, worktreeID: backgroundWorktreeID)
        let backgroundPaneID = appState.workspaceRoots[backgroundKey]!
            .allAreas()[0]
            .tabs[0]
            .content
            .pane!
            .id

        let view = TerminalViewMaterializer.ensureMaterialized(
            paneID: backgroundPaneID,
            appState: appState
        )

        let envKeys = Set(view?.envVars.map(\.key) ?? [])
        #expect(envKeys.contains("MUXY_PANE_ID"))
        #expect(envKeys.contains("MUXY_PROJECT_ID"))
        #expect(envKeys.contains("MUXY_WORKTREE_ID"))
    }

    private func makeAppState() -> AppState {
        AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
    }

    private func makeAppStateWithWorktrees(
        projectID: UUID,
        activeWorktreeID: UUID,
        backgroundWorktreeID: UUID
    ) -> AppState {
        let appState = makeAppState()
        let activeKey = WorktreeKey(projectID: projectID, worktreeID: activeWorktreeID)
        let backgroundKey = WorktreeKey(projectID: projectID, worktreeID: backgroundWorktreeID)
        let activeArea = TabArea(projectPath: "/tmp/active")
        let backgroundArea = TabArea(projectPath: "/tmp/background")

        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = activeWorktreeID
        appState.workspaceRoots[activeKey] = .tabArea(activeArea)
        appState.workspaceRoots[backgroundKey] = .tabArea(backgroundArea)
        appState.focusedAreaID[activeKey] = activeArea.id
        appState.focusedAreaID[backgroundKey] = backgroundArea.id
        return appState
    }
}

private final class WorkspacePersistenceStub: WorkspacePersisting {
    private var snapshots: [WorkspaceSnapshot] = []
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { snapshots }
    func saveWorkspaces(_ workspaces: [WorkspaceSnapshot]) throws { snapshots = workspaces }
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
