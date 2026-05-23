import Foundation
import Testing

@testable import Muxy

@Suite("createBrowserTabInWorktree")
@MainActor
struct CreateBrowserTabInWorktreeTests {
    @Test("opens the browser tab in the originating worktree, not the project's active worktree")
    func opensTabInOriginatingWorktree() {
        let harness = makeTwoWorktreeHarness()

        harness.appState.dispatch(.createBrowserTabInWorktree(
            worktreeKey: harness.backgroundKey,
            areaID: harness.backgroundArea.id,
            initialURL: "https://example.com"
        ))

        #expect(harness.activeArea.tabs.allSatisfy { $0.kind != .browser })
        let backgroundBrowserTabs = harness.backgroundArea.tabs.filter { $0.kind == .browser }
        #expect(backgroundBrowserTabs.count == 1)
        let session = backgroundBrowserTabs.first?.content.browserSession
        #expect(session?.nav.currentURL == "https://example.com")
    }

    @Test("falls back to focused area when areaID is nil")
    func usesFocusedAreaWhenNoneProvided() {
        let harness = makeTwoWorktreeHarness()

        harness.appState.dispatch(.createBrowserTabInWorktree(
            worktreeKey: harness.backgroundKey,
            areaID: nil,
            initialURL: nil
        ))

        let backgroundBrowserTabs = harness.backgroundArea.tabs.filter { $0.kind == .browser }
        #expect(backgroundBrowserTabs.count == 1)
    }

    @Test("ignores requests with unknown worktree keys")
    func ignoresUnknownWorktreeKey() {
        let harness = makeTwoWorktreeHarness()
        let unknownKey = WorktreeKey(projectID: harness.projectID, worktreeID: UUID())

        harness.appState.dispatch(.createBrowserTabInWorktree(
            worktreeKey: unknownKey,
            areaID: nil,
            initialURL: "https://example.com"
        ))

        #expect(harness.activeArea.tabs.allSatisfy { $0.kind != .browser })
        #expect(harness.backgroundArea.tabs.allSatisfy { $0.kind != .browser })
    }

    private func makeTwoWorktreeHarness() -> TwoWorktreeHarness {
        let projectID = UUID()
        let activeWorktreeID = UUID()
        let backgroundWorktreeID = UUID()
        let activeKey = WorktreeKey(projectID: projectID, worktreeID: activeWorktreeID)
        let backgroundKey = WorktreeKey(projectID: projectID, worktreeID: backgroundWorktreeID)
        let activeArea = TabArea(projectPath: "/tmp/active")
        let backgroundArea = TabArea(projectPath: "/tmp/background")
        let appState = AppState(
            selectionStore: BrowserTabSelectionStoreStub(),
            terminalViews: BrowserTabTerminalViewRemovingStub(),
            workspacePersistence: BrowserTabWorkspacePersistenceStub()
        )
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = activeWorktreeID
        appState.workspaceRoots[activeKey] = .tabArea(activeArea)
        appState.workspaceRoots[backgroundKey] = .tabArea(backgroundArea)
        appState.focusedAreaID[activeKey] = activeArea.id
        appState.focusedAreaID[backgroundKey] = backgroundArea.id
        return TwoWorktreeHarness(
            appState: appState,
            projectID: projectID,
            activeKey: activeKey,
            backgroundKey: backgroundKey,
            activeArea: activeArea,
            backgroundArea: backgroundArea
        )
    }

    private struct TwoWorktreeHarness {
        let appState: AppState
        let projectID: UUID
        let activeKey: WorktreeKey
        let backgroundKey: WorktreeKey
        let activeArea: TabArea
        let backgroundArea: TabArea
    }
}

private final class BrowserTabWorkspacePersistenceStub: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
}

@MainActor
private final class BrowserTabSelectionStoreStub: ActiveProjectSelectionStoring {
    func loadActiveProjectID() -> UUID? { nil }
    func saveActiveProjectID(_: UUID?) {}
    func loadActiveWorktreeIDs() -> [UUID: UUID] { [:] }
    func saveActiveWorktreeIDs(_: [UUID: UUID]) {}
}

@MainActor
private final class BrowserTabTerminalViewRemovingStub: TerminalViewRemoving {
    func removeView(for _: UUID) {}
    func needsConfirmQuit(for _: UUID) -> Bool { false }
}
