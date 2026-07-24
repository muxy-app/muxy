import Foundation
import Testing

@testable import Muxy

@Suite("AppState top-level tabs")
@MainActor
struct AppStateTopLevelTabTests {
    @Test("global tab reorder remains independent from docked group membership")
    func globalTabReorderPreservesDockedGroups() {
        let projectID = UUID()
        let worktreeID = UUID()
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let appState = makeAppState(projectID: projectID, worktreeID: worktreeID)
        let areaID = appState.focusedAreaID[key]!
        let firstTabID = appState.workspaceRoots[key]!.findArea(id: areaID)!.activeTabID!
        appState.dispatch(.createTab(projectID: projectID, areaID: areaID))
        let secondTabID = appState.workspaceRoots[key]!.findArea(id: areaID)!.activeTabID!
        let groupID = appState.topLevelTabLayouts[key]!.allGroups()[0].id
        appState.dispatch(.moveTopLevelTab(
            projectID: projectID,
            request: .toNewSplit(
                tabID: secondTabID,
                sourceGroupID: groupID,
                targetGroupID: groupID,
                split: SplitPlacement(direction: .horizontal, position: .second)
            )
        ))

        appState.reorderTopLevelTabs(
            for: key,
            fromOffsets: IndexSet(integer: 1),
            toOffset: 0
        )
        appState.dispatch(.selectTab(
            projectID: projectID,
            areaID: areaID,
            tabID: firstTabID
        ))

        #expect(appState.topLevelTabs(for: key).map(\.tab.id) == [secondTabID, firstTabID])
        #expect(appState.topLevelTabShortcutIndices(for: key)[secondTabID] == 0)
        #expect(appState.topLevelTabShortcutIndices(for: key)[firstTabID] == 1)
        let groups = appState.topLevelTabLayouts[key]!.allGroups()
        #expect(groups.count == 2)
        #expect(groups.contains { $0.tabIDs == [firstTabID] })
        #expect(groups.contains { $0.tabIDs == [secondTabID] })
    }

    @Test("empty workspace retains an area for its tab strip actions")
    func emptyWorkspaceRetainsTabStripArea() {
        let projectID = UUID()
        let worktreeID = UUID()
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let appState = makeAppState(projectID: projectID, worktreeID: worktreeID)
        let area = appState.workspaceRoots[key]!.allAreas()[0]
        _ = area.extractTabForMove(area.activeTabID!)
        let group = TopLevelTabGroup(tabIDs: [], activeTabID: nil)
        appState.topLevelTabOrder[key] = []
        appState.topLevelTabLayouts[key] = .group(group)

        #expect(appState.tabStripAreaID(for: key, groupID: group.id) == area.id)
    }

    private func makeAppState(projectID: UUID, worktreeID: UUID) -> AppState {
        let appState = AppState(
            selectionStore: TopLevelTabSelectionStoreStub(),
            terminalViews: TopLevelTabTerminalViewRemovingStub(),
            workspacePersistence: TopLevelTabWorkspacePersistenceStub()
        )
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = TabArea(projectPath: "/tmp/test")
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = .tabArea(area)
        appState.focusedAreaID[key] = area.id
        return appState
    }
}

private final class TopLevelTabWorkspacePersistenceStub: WorkspacePersisting {
    private var snapshots: [WorkspaceSnapshot] = []
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { snapshots }
    func saveWorkspaces(_ workspaces: [WorkspaceSnapshot]) throws { snapshots = workspaces }
}

@MainActor
private final class TopLevelTabSelectionStoreStub: ActiveProjectSelectionStoring {
    private var activeProjectID: UUID?
    private var activeWorktreeIDs: [UUID: UUID] = [:]
    func loadActiveProjectID() -> UUID? { activeProjectID }
    func saveActiveProjectID(_ id: UUID?) { activeProjectID = id }
    func loadActiveWorktreeIDs() -> [UUID: UUID] { activeWorktreeIDs }
    func saveActiveWorktreeIDs(_ ids: [UUID: UUID]) { activeWorktreeIDs = ids }
}

@MainActor
private final class TopLevelTabTerminalViewRemovingStub: TerminalViewRemoving {
    func removeView(for paneID: UUID) {}
    func needsConfirmQuit(for paneID: UUID) -> Bool { false }
}
