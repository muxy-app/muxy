import Foundation
import Testing

@testable import Muxy

@Suite("AppState maximize panes")
@MainActor
struct AppStateMaximizeTests {
    @Test("maximized pane blocks directional focus into hidden panes")
    func maximizedPaneBlocksDirectionalFocusIntoHiddenPanes() {
        let projectID = UUID()
        let worktreeID = UUID()
        let appState = makeAppState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let (_, secondAreaID) = splitWorkspace(appState, projectID: projectID, key: key)

        appState.toggleMaximize(areaID: secondAreaID, for: projectID)
        appState.dispatch(.focusPaneLeft(projectID: projectID))

        #expect(appState.focusedAreaID[key] == secondAreaID)
        #expect(appState.maximizedPanes[key]?.areaID == secondAreaID)
    }

    @Test("cross-pane tab cycling restores full layout when focus leaves maximized pane")
    func crossPaneTabCyclingRestoresFullLayoutWhenFocusLeavesMaximizedPane() {
        let projectID = UUID()
        let worktreeID = UUID()
        let appState = makeAppState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let (firstAreaID, secondAreaID) = splitWorkspace(appState, projectID: projectID, key: key)

        appState.toggleMaximize(areaID: secondAreaID, for: projectID)
        appState.dispatch(.cycleNextTabAcrossPanes(projectID: projectID))

        #expect(appState.focusedAreaID[key] == firstAreaID)
        #expect(appState.maximizedPanes[key] == nil)
    }

    @Test("reverse cross-pane tab cycling restores full layout when focus leaves maximized pane")
    func reverseCrossPaneTabCyclingRestoresFullLayoutWhenFocusLeavesMaximizedPane() {
        let projectID = UUID()
        let worktreeID = UUID()
        let appState = makeAppState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let (firstAreaID, secondAreaID) = splitWorkspace(appState, projectID: projectID, key: key)

        appState.toggleMaximize(areaID: firstAreaID, for: projectID)
        appState.dispatch(.cyclePreviousTabAcrossPanes(projectID: projectID))

        #expect(appState.focusedAreaID[key] == secondAreaID)
        #expect(appState.maximizedPanes[key] == nil)
    }

    @Test("selecting the active top-level tab by index preserves the maximized child")
    func selectingActiveTopLevelTabPreservesMaximizedChild() {
        let projectID = UUID()
        let worktreeID = UUID()
        let appState = makeAppState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let (firstAreaID, secondAreaID) = splitWorkspace(appState, projectID: projectID, key: key)
        let root = appState.workspaceRoots[key]!
        let firstArea = root.findArea(id: firstAreaID)!
        let secondArea = root.findArea(id: secondAreaID)!
        let firstRootTabID = firstArea.tabs[0].id
        let childTabID = secondArea.tabs[0].id

        appState.toggleMaximize(areaID: secondAreaID, for: projectID)
        appState.selectTabByIndex(0, projectID: projectID)

        #expect(appState.focusedAreaID[key] == secondAreaID)
        #expect(appState.maximizedPanes[key]?.areaID == secondAreaID)
        #expect(appState.maximizedPanes[key]?.topLevelTabID == firstRootTabID)
        #expect(firstArea.activeTabID == firstRootTabID)
        #expect(secondArea.activeTabID == childTabID)
    }

    @Test("direct area focus restores full layout when focus leaves maximized pane")
    func directAreaFocusRestoresFullLayoutWhenFocusLeavesMaximizedPane() {
        let projectID = UUID()
        let worktreeID = UUID()
        let appState = makeAppState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let (firstAreaID, secondAreaID) = splitWorkspace(appState, projectID: projectID, key: key)

        appState.toggleMaximize(areaID: secondAreaID, for: projectID)
        appState.dispatch(.focusArea(projectID: projectID, areaID: firstAreaID))

        #expect(appState.focusedAreaID[key] == firstAreaID)
        #expect(appState.maximizedPanes[key] == nil)
    }

    @Test("maximize guard uses action project")
    func maximizeGuardUsesActionProject() {
        let firstProjectID = UUID()
        let firstWorktreeID = UUID()
        let secondProjectID = UUID()
        let secondWorktreeID = UUID()
        let appState = makeAppState(projectID: firstProjectID, worktreeID: firstWorktreeID)
        let firstKey = WorktreeKey(projectID: firstProjectID, worktreeID: firstWorktreeID)
        let secondKey = WorktreeKey(projectID: secondProjectID, worktreeID: secondWorktreeID)
        let (_, firstProjectSecondAreaID) = splitWorkspace(appState, projectID: firstProjectID, key: firstKey)
        let secondProjectFirstArea = TabArea(projectPath: "/tmp/test-2")

        appState.activeWorktreeID[secondProjectID] = secondWorktreeID
        appState.workspaceRoots[secondKey] = .tabArea(secondProjectFirstArea)
        appState.focusedAreaID[secondKey] = secondProjectFirstArea.id
        appState.dispatch(.splitArea(.init(
            projectID: secondProjectID,
            areaID: secondProjectFirstArea.id,
            direction: .horizontal,
            position: .second
        )))
        let secondProjectSecondAreaID = appState.focusedAreaID[secondKey]!
        appState.toggleMaximize(areaID: firstProjectSecondAreaID, for: firstProjectID)

        appState.dispatch(.focusPaneLeft(projectID: secondProjectID))

        #expect(appState.focusedAreaID[firstKey] == firstProjectSecondAreaID)
        #expect(appState.maximizedPanes[firstKey]?.areaID == firstProjectSecondAreaID)
        #expect(appState.focusedAreaID[secondKey] == secondProjectFirstArea.id)
        #expect(secondProjectSecondAreaID != secondProjectFirstArea.id)
    }

    @Test("single area workspace does not maximize")
    func singleAreaWorkspaceDoesNotMaximize() {
        let projectID = UUID()
        let worktreeID = UUID()
        let appState = makeAppState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let areaID = appState.focusedAreaID[key]!

        appState.toggleMaximize(areaID: areaID, for: projectID)

        #expect(appState.maximizedPanes[key] == nil)
    }

    @Test("splitting a maximized pane restores the full layout")
    func splittingMaximizedPaneRestoresFullLayout() {
        let projectID = UUID()
        let worktreeID = UUID()
        let appState = makeAppState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let (_, secondAreaID) = splitWorkspace(appState, projectID: projectID, key: key)

        appState.toggleMaximize(areaID: secondAreaID, for: projectID)
        appState.dispatch(.splitArea(.init(
            projectID: projectID,
            areaID: secondAreaID,
            direction: .horizontal,
            position: .second
        )))

        #expect(appState.maximizedPanes[key] == nil)
    }

    @Test("maximize distinguishes docked parents sharing the same pane area")
    func maximizeDistinguishesDockedParentsSharingArea() {
        let projectID = UUID()
        let worktreeID = UUID()
        let appState = makeAppState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let rootAreaID = appState.focusedAreaID[key]!
        let firstTabID = appState.workspaceRoots[key]!.findArea(id: rootAreaID)!.activeTabID!
        appState.dispatch(.createTab(projectID: projectID, areaID: rootAreaID))
        let secondTabID = appState.workspaceRoots[key]!.findArea(id: rootAreaID)!.activeTabID!

        appState.dispatch(.selectTab(projectID: projectID, areaID: rootAreaID, tabID: firstTabID))
        appState.dispatch(.splitArea(.init(
            projectID: projectID,
            areaID: rootAreaID,
            direction: .horizontal,
            position: .second
        )))
        appState.dispatch(.selectTab(projectID: projectID, areaID: rootAreaID, tabID: secondTabID))
        appState.dispatch(.splitArea(.init(
            projectID: projectID,
            areaID: rootAreaID,
            direction: .vertical,
            position: .second
        )))

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

        appState.toggleMaximize(
            areaID: rootAreaID,
            topLevelTabID: secondTabID,
            for: projectID
        )
        #expect(appState.maximizedPanes[key] == AppState.MaximizedPane(
            topLevelTabID: secondTabID,
            areaID: rootAreaID
        ))

        appState.toggleMaximize(
            areaID: rootAreaID,
            topLevelTabID: firstTabID,
            for: projectID
        )
        #expect(appState.maximizedPanes[key] == AppState.MaximizedPane(
            topLevelTabID: firstTabID,
            areaID: rootAreaID
        ))
    }

    private func makeAppState(projectID: UUID, worktreeID: UUID) -> AppState {
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = TabArea(projectPath: "/tmp/test")
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = .tabArea(area)
        appState.focusedAreaID[key] = area.id
        return appState
    }

    private func splitWorkspace(
        _ appState: AppState,
        projectID: UUID,
        key: WorktreeKey
    ) -> (firstAreaID: UUID, secondAreaID: UUID) {
        let firstAreaID = appState.focusedAreaID[key]!
        appState.dispatch(.splitArea(.init(
            projectID: projectID,
            areaID: firstAreaID,
            direction: .horizontal,
            position: .second
        )))
        return (firstAreaID, appState.focusedAreaID[key]!)
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
