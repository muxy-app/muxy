import Foundation
import Testing

@testable import Muxy

@Suite("WorkspaceReducer")
@MainActor
struct WorkspaceReducerTests {
    private let testPath = "/tmp/test"

    private func makeState(
        projectID: UUID,
        worktreeID: UUID,
        worktreePath: String = "/tmp/test"
    ) -> WorkspaceState {
        var state = WorkspaceState(
            activeProjectID: projectID,
            activeWorktreeID: [projectID: worktreeID],
            workspaceRoots: [:],
            focusedAreaID: [:],
            focusHistory: [:]
        )
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = TabArea(projectPath: worktreePath)
        state.workspaceRoots[key] = .tabArea(area)
        state.focusedAreaID[key] = area.id
        return state
    }

    private func focusedArea(in state: WorkspaceState, projectID: UUID) -> TabArea? {
        guard let worktreeID = state.activeWorktreeID[projectID] else { return nil }
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        guard let focusedID = state.focusedAreaID[key],
              let root = state.workspaceRoots[key]
        else { return nil }
        return root.findArea(id: focusedID)
    }

    private func area(in state: WorkspaceState, key: WorktreeKey, areaID: UUID) -> TabArea? {
        state.workspaceRoots[key]?.findArea(id: areaID)
    }

    @Test("selectProject creates workspace if new")
    func selectProjectNew() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = WorkspaceState(
            activeProjectID: nil,
            activeWorktreeID: [:],
            workspaceRoots: [:],
            focusedAreaID: [:],
            focusHistory: [:]
        )
        let action = AppState.Action.selectProject(
            projectID: projectID, worktreeID: worktreeID, worktreePath: testPath
        )
        _ = WorkspaceReducer.reduce(action: action, state: &state)

        #expect(state.activeProjectID == projectID)
        #expect(state.activeWorktreeID[projectID] == worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        #expect(state.workspaceRoots[key] != nil)
        #expect(state.focusedAreaID[key] != nil)
    }

    @Test("selectProject existing workspace does not recreate")
    func selectProjectExisting() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let originalAreaID = state.focusedAreaID[key]

        let action = AppState.Action.selectProject(
            projectID: projectID, worktreeID: worktreeID, worktreePath: testPath
        )
        _ = WorkspaceReducer.reduce(action: action, state: &state)

        #expect(state.focusedAreaID[key] == originalAreaID)
    }

    @Test("selectWorktree creates workspace if new")
    func selectWorktreeNew() {
        let projectID = UUID()
        let worktreeID = UUID()
        let newWorktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)

        let action = AppState.Action.selectWorktree(
            projectID: projectID, worktreeID: newWorktreeID, worktreePath: "/tmp/other"
        )
        _ = WorkspaceReducer.reduce(action: action, state: &state)

        #expect(state.activeWorktreeID[projectID] == newWorktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: newWorktreeID)
        #expect(state.workspaceRoots[key] != nil)
    }

    @Test("removeProject clears all state and populates effects")
    func removeProject() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)

        let action = AppState.Action.removeProject(projectID: projectID)
        let effects = WorkspaceReducer.reduce(action: action, state: &state)

        #expect(state.activeProjectID == nil)
        #expect(state.activeWorktreeID[projectID] == nil)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        #expect(state.workspaceRoots[key] == nil)
        #expect(state.focusedAreaID[key] == nil)
        #expect(!effects.paneIDsToRemove.isEmpty)
    }

    @Test("removeWorktree with replacement switches to replacement")
    func removeWorktreeWithReplacement() {
        let projectID = UUID()
        let worktreeID = UUID()
        let replacementID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)

        let action = AppState.Action.removeWorktree(
            projectID: projectID,
            worktreeID: worktreeID,
            replacementWorktreeID: replacementID,
            replacementWorktreePath: "/tmp/replacement"
        )
        let effects = WorkspaceReducer.reduce(action: action, state: &state)

        #expect(state.activeWorktreeID[projectID] == replacementID)
        let newKey = WorktreeKey(projectID: projectID, worktreeID: replacementID)
        #expect(state.workspaceRoots[newKey] != nil)
        #expect(!effects.paneIDsToRemove.isEmpty)
    }

    @Test("removeWorktree without replacement clears project")
    func removeWorktreeNoReplacement() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)

        let action = AppState.Action.removeWorktree(
            projectID: projectID,
            worktreeID: worktreeID,
            replacementWorktreeID: nil,
            replacementWorktreePath: nil
        )
        _ = WorkspaceReducer.reduce(action: action, state: &state)

        #expect(state.activeProjectID == nil)
        #expect(state.activeWorktreeID[projectID] == nil)
    }

    @Test("createTab adds tab to focused area")
    func createTab() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)

        let action = AppState.Action.createTab(projectID: projectID, areaID: nil)
        let effects = WorkspaceReducer.reduce(action: action, state: &state)

        let area = focusedArea(in: state, projectID: projectID)
        #expect(area?.tabs.count == 2)
        #expect(effects.createdTabID == area?.tabs[1].id)
    }

    @Test("workspace-local actions reconcile only their worktree")
    func workspaceLocalActionReconcilesOnlyItsWorktree() {
        let projectID = UUID()
        let activeWorktreeID = UUID()
        let inactiveWorktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: activeWorktreeID)
        let inactiveKey = WorktreeKey(projectID: projectID, worktreeID: inactiveWorktreeID)
        let inactiveArea = TabArea(projectPath: "/tmp/inactive")
        let inactiveTabID = inactiveArea.activeTabID!
        state.workspaceRoots[inactiveKey] = .tabArea(inactiveArea)
        state.focusedAreaID[inactiveKey] = inactiveArea.id
        state.topLevelTabOrder[inactiveKey] = []
        state.topLevelTabLayouts[inactiveKey] = .group(TopLevelTabGroup(
            tabIDs: [inactiveTabID],
            activeTabID: inactiveTabID
        ))

        _ = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: nil),
            state: &state
        )

        #expect(state.topLevelTabOrder[inactiveKey] == [])
    }

    @Test("createExtensionTab adds extension tab and reports the instance id")
    func createExtensionTab() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)

        let action = extensionTabAction(projectID: projectID, data: .object(["pr": .number(1)]), singleton: false)
        let effects = WorkspaceReducer.reduce(action: action, state: &state)

        let area = focusedArea(in: state, projectID: projectID)
        #expect(area?.activeTab?.kind == .extensionWebView)
        #expect(area?.activeTab?.content.extensionState?.data == .object(["pr": .number(1)]))
        #expect(effects.createdTabID == area?.activeTab?.content.extensionState?.id)
    }

    @Test("non-singleton extension tab opens a duplicate every time")
    func createExtensionTabDuplicates() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)

        let action = extensionTabAction(projectID: projectID, data: nil, singleton: false)
        _ = WorkspaceReducer.reduce(action: action, state: &state)
        _ = WorkspaceReducer.reduce(action: action, state: &state)

        let area = focusedArea(in: state, projectID: projectID)
        #expect(area?.tabs.filter { $0.kind == .extensionWebView }.count == 2)
    }

    @Test("singleton extension tab focuses existing tab and reloads its data")
    func createExtensionTabSingletonReuses() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)

        let first = extensionTabAction(projectID: projectID, data: .object(["pr": .number(1)]), singleton: true)
        _ = WorkspaceReducer.reduce(action: first, state: &state)
        let area = focusedArea(in: state, projectID: projectID)
        let firstTabID = area?.activeTabID

        let reusedInstanceID = area?.activeTab?.content.extensionState?.id

        area?.createTab()

        let second = extensionTabAction(projectID: projectID, data: .object(["pr": .number(2)]), singleton: true)
        let effects = WorkspaceReducer.reduce(action: second, state: &state)

        #expect(area?.tabs.filter { $0.kind == .extensionWebView }.count == 1)
        #expect(area?.activeTabID == firstTabID)
        #expect(area?.activeTab?.content.extensionState?.data == .object(["pr": .number(2)]))
        #expect(effects.createdTabID == reusedInstanceID)
    }

    private func extensionTabAction(
        projectID: UUID,
        data: ExtensionJSON?,
        singleton: Bool
    ) -> AppState.Action {
        .createExtensionTab(
            projectID: projectID,
            areaID: nil,
            request: AppState.CreateExtensionTabRequest(
                extensionID: "pr-tools",
                tabTypeID: "pr-viewer",
                title: "PR Viewer",
                data: data,
                singleton: singleton
            )
        )
    }

    @Test("closeTab removes tab and populates paneIDsToRemove")
    func closeTab() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let areaID = state.focusedAreaID[key]!
        state.focusHistory[key] = [areaID]

        _ = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: nil),
            state: &state
        )
        let area = focusedArea(in: state, projectID: projectID)!
        let firstTabID = area.tabs[0].id

        let effects = WorkspaceReducer.reduce(
            action: .closeTab(projectID: projectID, areaID: areaID, tabID: firstTabID),
            state: &state
        )
        #expect(area.tabs.count == 1)
        #expect(!effects.paneIDsToRemove.isEmpty)
    }

    @Test("closing a top-level tab closes its child panes")
    func closeTabLastInMultiArea() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let firstAreaID = state.focusedAreaID[key]!

        _ = WorkspaceReducer.reduce(
            action: .splitArea(AppState.SplitAreaRequest(
                projectID: projectID,
                areaID: firstAreaID,
                direction: .horizontal,
                position: .second
            )),
            state: &state
        )

        let firstArea = state.workspaceRoots[key]!.findArea(id: firstAreaID)!
        let tabID = firstArea.tabs[0].id

        let effects = WorkspaceReducer.reduce(
            action: .closeTab(projectID: projectID, areaID: firstAreaID, tabID: tabID),
            state: &state
        )

        #expect(state.workspaceRoots[key] == nil)
        #expect(effects.paneIDsToRemove.count == 2)
    }

    @Test("closing a pinned top-level tab preserves its child panes")
    func closePinnedTopLevelTabPreservesChildPanes() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let rootAreaID = state.focusedAreaID[key]!

        _ = WorkspaceReducer.reduce(
            action: .splitArea(AppState.SplitAreaRequest(
                projectID: projectID,
                areaID: rootAreaID,
                direction: .horizontal,
                position: .second
            )),
            state: &state
        )

        let rootArea = state.workspaceRoots[key]!.findArea(id: rootAreaID)!
        let rootTab = rootArea.tabs[0]
        rootArea.togglePin(rootTab.id)

        let effects = WorkspaceReducer.reduce(
            action: .closeTab(projectID: projectID, areaID: rootAreaID, tabID: rootTab.id),
            state: &state
        )

        #expect(state.workspaceRoots[key]!.allAreas().count == 2)
        #expect(state.workspaceRoots[key]!.allTabs().count == 2)
        #expect(effects.paneIDsToRemove.isEmpty)
    }

    @Test("closeTab last tab in last area triggers projectIDsToRemove")
    func closeTabLastInLastArea() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let areaID = state.focusedAreaID[key]!
        let area = state.workspaceRoots[key]!.findArea(id: areaID)!
        let tabID = area.tabs[0].id

        let effects = WorkspaceReducer.reduce(
            action: .closeTab(projectID: projectID, areaID: areaID, tabID: tabID),
            state: &state
        )

        #expect(state.workspaceRoots[key] == nil)
        #expect(state.focusHistory[key] == nil)
        #expect(effects.projectIDsToRemove.contains(projectID))
    }

    @Test("closeTab last tab keeps empty workspace when keepProjectOpenWhenEmpty is on")
    func closeTabLastTabKeepsEmptyWorkspace() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        state.keepProjectOpenWhenEmpty = true
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let areaID = state.focusedAreaID[key]!
        let area = state.workspaceRoots[key]!.findArea(id: areaID)!
        let tabID = area.tabs[0].id

        let effects = WorkspaceReducer.reduce(
            action: .closeTab(projectID: projectID, areaID: areaID, tabID: tabID),
            state: &state
        )

        #expect(state.workspaceRoots[key] != nil)
        #expect(state.workspaceRoots[key]?.findArea(id: areaID)?.tabs.isEmpty == true)
        #expect(!effects.projectIDsToRemove.contains(projectID))
    }

    @Test("selectProject does not create a tab for an emptied workspace")
    func selectProjectKeepsEmptyWorkspace() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        state.keepProjectOpenWhenEmpty = true
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let areaID = state.focusedAreaID[key]!
        let tabID = state.workspaceRoots[key]!.findArea(id: areaID)!.tabs[0].id

        _ = WorkspaceReducer.reduce(
            action: .closeTab(projectID: projectID, areaID: areaID, tabID: tabID),
            state: &state
        )
        _ = WorkspaceReducer.reduce(
            action: .selectProject(projectID: projectID, worktreeID: worktreeID, worktreePath: testPath),
            state: &state
        )

        #expect(state.workspaceRoots[key]?.findArea(id: areaID)?.tabs.isEmpty == true)
    }

    @Test("selectTab changes activeTabID")
    func selectTab() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let areaID = state.focusedAreaID[key]!

        _ = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: nil),
            state: &state
        )
        let area = focusedArea(in: state, projectID: projectID)!
        let firstTabID = area.tabs[0].id

        _ = WorkspaceReducer.reduce(
            action: .selectTab(projectID: projectID, areaID: areaID, tabID: firstTabID),
            state: &state
        )
        #expect(area.activeTabID == firstTabID)
    }

    @Test("selectNextTab cycles through tabs")
    func selectNextTab() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)

        _ = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: nil),
            state: &state
        )
        let area = focusedArea(in: state, projectID: projectID)!
        let firstTabID = area.tabs[0].id

        _ = WorkspaceReducer.reduce(
            action: .selectNextTab(projectID: projectID),
            state: &state
        )
        #expect(area.activeTabID == firstTabID)
    }

    @Test("splitArea creates split and focuses new area")
    func splitArea() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let originalAreaID = state.focusedAreaID[key]!

        let effects = WorkspaceReducer.reduce(
            action: .splitArea(AppState.SplitAreaRequest(
                projectID: projectID,
                areaID: originalAreaID,
                direction: .horizontal,
                position: .second
            )),
            state: &state
        )

        let root = state.workspaceRoots[key]!
        #expect(root.allAreas().count == 2)
        #expect(state.focusedAreaID[key] != originalAreaID)
        let newArea = root.findArea(id: state.focusedAreaID[key]!)
        #expect(effects.createdPaneID == newArea?.tabs.first?.content.pane?.id)
    }

    @Test("closeArea removes area and focuses from history")
    func closeArea() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let firstAreaID = state.focusedAreaID[key]!

        _ = WorkspaceReducer.reduce(
            action: .splitArea(AppState.SplitAreaRequest(
                projectID: projectID,
                areaID: firstAreaID,
                direction: .horizontal,
                position: .second
            )),
            state: &state
        )
        let newAreaID = state.focusedAreaID[key]!

        _ = WorkspaceReducer.reduce(
            action: .closeArea(projectID: projectID, areaID: newAreaID),
            state: &state
        )

        #expect(state.focusedAreaID[key] == firstAreaID)
        #expect(state.workspaceRoots[key]!.allAreas().count == 1)
    }

    @Test("closeArea last area clears workspace and triggers projectIDsToRemove")
    func closeAreaLast() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let areaID = state.focusedAreaID[key]!

        let effects = WorkspaceReducer.reduce(
            action: .closeArea(projectID: projectID, areaID: areaID),
            state: &state
        )

        #expect(state.workspaceRoots[key] == nil)
        #expect(effects.projectIDsToRemove.contains(projectID))
    }

    @Test("focusArea updates focusedAreaID and maintains history")
    func focusArea() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let firstAreaID = state.focusedAreaID[key]!

        _ = WorkspaceReducer.reduce(
            action: .splitArea(AppState.SplitAreaRequest(
                projectID: projectID,
                areaID: firstAreaID,
                direction: .horizontal,
                position: .second
            )),
            state: &state
        )
        let secondAreaID = state.focusedAreaID[key]!

        _ = WorkspaceReducer.reduce(
            action: .focusArea(projectID: projectID, areaID: firstAreaID),
            state: &state
        )

        #expect(state.focusedAreaID[key] == firstAreaID)
        #expect(state.focusHistory[key]?.contains(secondAreaID) == true)
    }

    @Test("focus history does not exceed 20 entries")
    func focusHistoryLimit() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let originalAreaID = state.focusedAreaID[key]!

        var areaIDs = [originalAreaID]
        for _ in 0 ..< 25 {
            let lastAreaID = state.focusedAreaID[key]!
            _ = WorkspaceReducer.reduce(
                action: .splitArea(AppState.SplitAreaRequest(
                    projectID: projectID,
                    areaID: lastAreaID,
                    direction: .horizontal,
                    position: .second
                )),
                state: &state
            )
            areaIDs.append(state.focusedAreaID[key]!)
        }

        let history = state.focusHistory[key] ?? []
        #expect(history.count <= 20)
    }

    @Test("focusPaneRight selects pane to the right")
    func focusPaneRight() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let leftAreaID = state.focusedAreaID[key]!

        _ = WorkspaceReducer.reduce(
            action: .splitArea(AppState.SplitAreaRequest(
                projectID: projectID,
                areaID: leftAreaID,
                direction: .horizontal,
                position: .second
            )),
            state: &state
        )
        let rightAreaID = state.focusedAreaID[key]!

        _ = WorkspaceReducer.reduce(
            action: .focusArea(projectID: projectID, areaID: leftAreaID),
            state: &state
        )
        #expect(state.focusedAreaID[key] == leftAreaID)

        _ = WorkspaceReducer.reduce(
            action: .focusPaneRight(projectID: projectID),
            state: &state
        )
        #expect(state.focusedAreaID[key] == rightAreaID)
    }

    @Test("focusPaneLeft selects pane to the left")
    func focusPaneLeft() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let leftAreaID = state.focusedAreaID[key]!

        _ = WorkspaceReducer.reduce(
            action: .splitArea(AppState.SplitAreaRequest(
                projectID: projectID,
                areaID: leftAreaID,
                direction: .horizontal,
                position: .second
            )),
            state: &state
        )
        let rightAreaID = state.focusedAreaID[key]!
        #expect(state.focusedAreaID[key] == rightAreaID)

        _ = WorkspaceReducer.reduce(
            action: .focusPaneLeft(projectID: projectID),
            state: &state
        )
        #expect(state.focusedAreaID[key] == leftAreaID)
    }

    @Test("focusPaneDown selects pane below")
    func focusPaneDown() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let topAreaID = state.focusedAreaID[key]!

        _ = WorkspaceReducer.reduce(
            action: .splitArea(AppState.SplitAreaRequest(
                projectID: projectID,
                areaID: topAreaID,
                direction: .vertical,
                position: .second
            )),
            state: &state
        )
        let bottomAreaID = state.focusedAreaID[key]!

        _ = WorkspaceReducer.reduce(
            action: .focusArea(projectID: projectID, areaID: topAreaID),
            state: &state
        )

        _ = WorkspaceReducer.reduce(
            action: .focusPaneDown(projectID: projectID),
            state: &state
        )
        #expect(state.focusedAreaID[key] == bottomAreaID)
    }

    @Test("cycleNextTabAcrossPanes walks the selected parent pane tree")
    func cycleNextTabAcrossPanesWalksSelectedParentTree() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let firstAreaID = state.focusedAreaID[key]!

        let firstTabID = area(in: state, key: key, areaID: firstAreaID)!.tabs[0].id

        _ = WorkspaceReducer.reduce(
            action: .splitArea(AppState.SplitAreaRequest(
                projectID: projectID,
                areaID: firstAreaID,
                direction: .horizontal,
                position: .second
            )),
            state: &state
        )
        let secondAreaID = state.focusedAreaID[key]!
        let secondAreaTabID = area(in: state, key: key, areaID: secondAreaID)!.tabs[0].id
        #expect(area(in: state, key: key, areaID: secondAreaID)?.tabs[0].parentTabID == firstTabID)

        _ = WorkspaceReducer.reduce(
            action: .selectTab(projectID: projectID, areaID: firstAreaID, tabID: firstTabID),
            state: &state
        )

        _ = WorkspaceReducer.reduce(
            action: .cycleNextTabAcrossPanes(projectID: projectID),
            state: &state
        )
        #expect(state.focusedAreaID[key] == secondAreaID)
        #expect(area(in: state, key: key, areaID: secondAreaID)?.activeTabID == secondAreaTabID)

        _ = WorkspaceReducer.reduce(
            action: .cycleNextTabAcrossPanes(projectID: projectID),
            state: &state
        )
        #expect(state.focusedAreaID[key] == firstAreaID)
    }

    @Test("cyclePreviousTabAcrossPanes walks backward across panes")
    func cyclePreviousTabAcrossPanesWalksBackward() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let firstAreaID = state.focusedAreaID[key]!

        let firstAreaTabID = area(in: state, key: key, areaID: firstAreaID)!.tabs[0].id

        _ = WorkspaceReducer.reduce(
            action: .splitArea(AppState.SplitAreaRequest(
                projectID: projectID,
                areaID: firstAreaID,
                direction: .horizontal,
                position: .second
            )),
            state: &state
        )
        let secondAreaID = state.focusedAreaID[key]!
        let secondAreaTabID = area(in: state, key: key, areaID: secondAreaID)!.tabs[0].id

        _ = WorkspaceReducer.reduce(
            action: .selectTab(projectID: projectID, areaID: secondAreaID, tabID: secondAreaTabID),
            state: &state
        )

        _ = WorkspaceReducer.reduce(
            action: .cyclePreviousTabAcrossPanes(projectID: projectID),
            state: &state
        )
        #expect(state.focusedAreaID[key] == firstAreaID)
        #expect(area(in: state, key: key, areaID: firstAreaID)?.activeTabID == firstAreaTabID)
    }

    @Test("cycleTabAcrossPanes wraps between first and last entries")
    func cycleTabAcrossPanesWrapsBetweenFirstAndLastEntries() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let firstAreaID = state.focusedAreaID[key]!

        let firstTabID = area(in: state, key: key, areaID: firstAreaID)!.tabs[0].id

        _ = WorkspaceReducer.reduce(
            action: .splitArea(AppState.SplitAreaRequest(
                projectID: projectID,
                areaID: firstAreaID,
                direction: .horizontal,
                position: .second
            )),
            state: &state
        )
        let secondAreaID = state.focusedAreaID[key]!
        let lastTabID = area(in: state, key: key, areaID: secondAreaID)!.tabs[0].id

        _ = WorkspaceReducer.reduce(
            action: .selectTab(projectID: projectID, areaID: secondAreaID, tabID: lastTabID),
            state: &state
        )
        _ = WorkspaceReducer.reduce(
            action: .cycleNextTabAcrossPanes(projectID: projectID),
            state: &state
        )
        #expect(state.focusedAreaID[key] == firstAreaID)
        #expect(area(in: state, key: key, areaID: firstAreaID)?.activeTabID == firstTabID)

        _ = WorkspaceReducer.reduce(
            action: .cyclePreviousTabAcrossPanes(projectID: projectID),
            state: &state
        )
        #expect(state.focusedAreaID[key] == secondAreaID)
        #expect(area(in: state, key: key, areaID: secondAreaID)?.activeTabID == lastTabID)
    }

    @Test("cycleTabAcrossPanes does nothing with one tab total")
    func cycleTabAcrossPanesDoesNothingWithOneTabTotal() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let areaID = state.focusedAreaID[key]!
        let tabID = area(in: state, key: key, areaID: areaID)!.activeTabID

        _ = WorkspaceReducer.reduce(
            action: .cycleNextTabAcrossPanes(projectID: projectID),
            state: &state
        )
        #expect(state.focusedAreaID[key] == areaID)
        #expect(area(in: state, key: key, areaID: areaID)?.activeTabID == tabID)

        _ = WorkspaceReducer.reduce(
            action: .cyclePreviousTabAcrossPanes(projectID: projectID),
            state: &state
        )
        #expect(state.focusedAreaID[key] == areaID)
        #expect(area(in: state, key: key, areaID: areaID)?.activeTabID == tabID)
    }

    @Test("moveTab toArea swaps panes within one parent")
    func moveTabToArea() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let firstAreaID = state.focusedAreaID[key]!

        _ = WorkspaceReducer.reduce(
            action: .splitArea(AppState.SplitAreaRequest(
                projectID: projectID,
                areaID: firstAreaID,
                direction: .horizontal,
                position: .second
            )),
            state: &state
        )
        let secondAreaID = state.focusedAreaID[key]!

        let sourceArea = state.workspaceRoots[key]!.findArea(id: firstAreaID)!
        let destinationArea = state.workspaceRoots[key]!.findArea(id: secondAreaID)!
        let tabToMove = sourceArea.tabs[0].id
        let destinationTabID = destinationArea.tabs[0].id

        _ = WorkspaceReducer.reduce(
            action: .moveTab(
                projectID: projectID,
                request: .toArea(tabID: tabToMove, sourceAreaID: firstAreaID, destinationAreaID: secondAreaID)
            ),
            state: &state
        )

        let destArea = state.workspaceRoots[key]!.findArea(id: secondAreaID)!
        #expect(destArea.tabs.contains(where: { $0.id == tabToMove }))
        #expect(sourceArea.tabs.contains(where: { $0.id == destinationTabID }))
    }

    @Test("moveTab toArea does not collapse either swapped pane")
    func moveTabToAreaDoesNotCollapse() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let firstAreaID = state.focusedAreaID[key]!

        _ = WorkspaceReducer.reduce(
            action: .splitArea(AppState.SplitAreaRequest(
                projectID: projectID,
                areaID: firstAreaID,
                direction: .horizontal,
                position: .second
            )),
            state: &state
        )
        let secondAreaID = state.focusedAreaID[key]!

        let sourceArea = state.workspaceRoots[key]!.findArea(id: firstAreaID)!
        let tabToMove = sourceArea.tabs[0].id

        let effects = WorkspaceReducer.reduce(
            action: .moveTab(
                projectID: projectID,
                request: .toArea(tabID: tabToMove, sourceAreaID: firstAreaID, destinationAreaID: secondAreaID)
            ),
            state: &state
        )

        let destArea = state.workspaceRoots[key]!.findArea(id: secondAreaID)!
        #expect(destArea.tabs.contains(where: { $0.id == tabToMove }))
        #expect(state.workspaceRoots[key]!.findArea(id: firstAreaID) != nil)
        #expect(!state.workspaceRoots[key]!.findArea(id: firstAreaID)!.tabs.isEmpty)
        #expect(effects.deferredAreaCollapses.isEmpty)
    }

    @Test("moveTab toNewSplit creates new split with tab")
    func moveTabToNewSplit() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let areaID = state.focusedAreaID[key]!

        _ = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: nil),
            state: &state
        )
        let area = focusedArea(in: state, projectID: projectID)!
        let tabToMove = area.tabs[0].id

        _ = WorkspaceReducer.reduce(
            action: .moveTab(
                projectID: projectID,
                request: .toNewSplit(
                    tabID: tabToMove,
                    sourceAreaID: areaID,
                    targetAreaID: areaID,
                    split: SplitPlacement(direction: .horizontal, position: .second)
                )
            ),
            state: &state
        )

        let root = state.workspaceRoots[key]!
        #expect(root.allAreas().count == 2)
    }

    @Test("moveTab toNewSplit moves a pinned pane")
    func movePinnedTabToNewSplit() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let areaID = state.focusedAreaID[key]!
        let area = focusedArea(in: state, projectID: projectID)!
        let tabToMove = area.tabs[0]
        tabToMove.isPinned = true

        _ = WorkspaceReducer.reduce(
            action: .moveTab(
                projectID: projectID,
                request: .toNewSplit(
                    tabID: tabToMove.id,
                    sourceAreaID: areaID,
                    targetAreaID: areaID,
                    split: SplitPlacement(direction: .horizontal, position: .second)
                )
            ),
            state: &state
        )

        let root = state.workspaceRoots[key]!
        #expect(root.allAreas().count == 2)
        #expect(root.locateTab(id: tabToMove.id)?.tab.isPinned == true)
    }

    @Test("moveTab toNewSplit defers collapse when source becomes empty")
    func moveTabToNewSplitDefersCollapse() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let sourceAreaID = state.focusedAreaID[key]!

        _ = WorkspaceReducer.reduce(
            action: .splitArea(AppState.SplitAreaRequest(
                projectID: projectID,
                areaID: sourceAreaID,
                direction: .horizontal,
                position: .second
            )),
            state: &state
        )
        let targetAreaID = state.focusedAreaID[key]!

        let sourceArea = state.workspaceRoots[key]!.findArea(id: sourceAreaID)!
        let tabToMove = sourceArea.tabs[0].id

        let effects = WorkspaceReducer.reduce(
            action: .moveTab(
                projectID: projectID,
                request: .toNewSplit(
                    tabID: tabToMove,
                    sourceAreaID: sourceAreaID,
                    targetAreaID: targetAreaID,
                    split: SplitPlacement(direction: .horizontal, position: .second)
                )
            ),
            state: &state
        )

        #expect(state.workspaceRoots[key]!.findArea(id: sourceAreaID) != nil)
        #expect(state.workspaceRoots[key]!.findArea(id: sourceAreaID)!.tabs.isEmpty)
        #expect(effects.deferredAreaCollapses.contains(where: { $0.areaID == sourceAreaID }))
    }

    @Test("selectNextProject cycles forward through projects")
    func selectNextProject() {
        let p1 = Project(name: "A", path: "/a")
        let p2 = Project(name: "B", path: "/b")
        let w1 = Worktree(name: "main", path: "/a", isPrimary: true)
        let w2 = Worktree(name: "main", path: "/b", isPrimary: true)

        var state = makeState(projectID: p1.id, worktreeID: w1.id, worktreePath: "/a")

        let worktrees: [UUID: [Worktree]] = [p1.id: [w1], p2.id: [w2]]

        _ = WorkspaceReducer.reduce(
            action: .selectNextProject(projects: [p1, p2], worktrees: worktrees),
            state: &state
        )
        #expect(state.activeProjectID == p2.id)
    }

    @Test("selectPreviousProject cycles backward")
    func selectPreviousProject() {
        let p1 = Project(name: "A", path: "/a")
        let p2 = Project(name: "B", path: "/b")
        let w1 = Worktree(name: "main", path: "/a", isPrimary: true)
        let w2 = Worktree(name: "main", path: "/b", isPrimary: true)

        var state = makeState(projectID: p1.id, worktreeID: w1.id, worktreePath: "/a")
        let worktrees: [UUID: [Worktree]] = [p1.id: [w1], p2.id: [w2]]

        _ = WorkspaceReducer.reduce(
            action: .selectPreviousProject(projects: [p1, p2], worktrees: worktrees),
            state: &state
        )
        #expect(state.activeProjectID == p2.id)
    }

    @Test("selectNextProject with single project is no-op")
    func selectNextProjectSingle() {
        let p1 = Project(name: "A", path: "/a")
        let w1 = Worktree(name: "main", path: "/a", isPrimary: true)
        var state = makeState(projectID: p1.id, worktreeID: w1.id, worktreePath: "/a")

        _ = WorkspaceReducer.reduce(
            action: .selectNextProject(projects: [p1], worktrees: [p1.id: [w1]]),
            state: &state
        )
        #expect(state.activeProjectID == p1.id)
    }

    @Test("selectTabByIndex selects correct tab")
    func selectTabByIndex() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)

        _ = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: nil),
            state: &state
        )
        _ = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: nil),
            state: &state
        )

        _ = WorkspaceReducer.reduce(
            action: .selectTabByIndex(projectID: projectID, index: 0),
            state: &state
        )

        let area = focusedArea(in: state, projectID: projectID)!
        #expect(area.activeTabID == area.tabs[0].id)
    }

    @Test("selectTabByIndex with negative index does nothing")
    func selectTabByIndexNegative() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)

        _ = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: nil),
            state: &state
        )

        let area = focusedArea(in: state, projectID: projectID)!
        let originalTabID = area.activeTabID

        _ = WorkspaceReducer.reduce(
            action: .selectTabByIndex(projectID: projectID, index: -1),
            state: &state
        )

        let newArea = focusedArea(in: state, projectID: projectID)!
        #expect(newArea.activeTabID == originalTabID)
    }

    @Test("selectTabByIndex ignores child panes")
    func selectTabByIndexIgnoresChildPanes() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)

        _ = WorkspaceReducer.reduce(action: .createTab(projectID: projectID, areaID: nil), state: &state)

        let firstAreaID = state.focusedAreaID[key]!

        _ = WorkspaceReducer.reduce(
            action: .splitArea(AppState.SplitAreaRequest(
                projectID: projectID,
                areaID: firstAreaID,
                direction: .horizontal,
                position: .second
            )),
            state: &state
        )

        let secondAreaID = state.focusedAreaID[key]!

        _ = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: secondAreaID),
            state: &state
        )

        let firstArea = area(in: state, key: key, areaID: firstAreaID)!
        let secondArea = area(in: state, key: key, areaID: secondAreaID)!

        #expect(firstArea.tabs.count == 2)
        #expect(secondArea.tabs.count == 2)

        let expectedTabID = secondArea.tabs[1].id
        _ = WorkspaceReducer.reduce(
            action: .selectTabByIndex(projectID: projectID, index: 2),
            state: &state
        )

        #expect(state.focusedAreaID[key] == secondAreaID)
        let newSecondArea = area(in: state, key: key, areaID: secondAreaID)!
        #expect(newSecondArea.activeTabID == expectedTabID)
    }

    @Test("splitting a child creates another direct child of the top-level tab")
    func splittingChildKeepsOneLevelOwnership() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let rootAreaID = state.focusedAreaID[key]!
        let rootTabID = state.workspaceRoots[key]!.findArea(id: rootAreaID)!.activeTabID!

        _ = WorkspaceReducer.reduce(
            action: .splitArea(.init(
                projectID: projectID,
                areaID: rootAreaID,
                direction: .horizontal,
                position: .second
            )),
            state: &state
        )
        let firstChildAreaID = state.focusedAreaID[key]!

        _ = WorkspaceReducer.reduce(
            action: .splitArea(.init(
                projectID: projectID,
                areaID: firstChildAreaID,
                direction: .vertical,
                position: .second
            )),
            state: &state
        )
        let secondChildAreaID = state.focusedAreaID[key]!
        let tabs = state.workspaceRoots[key]!.allTabs()

        #expect(tabs.count == 3)
        #expect(tabs.filter { $0.parentTabID == rootTabID }.count == 2)
        #expect(state.workspaceRoots[key]!.findArea(id: secondChildAreaID)?.activeTab?.parentTabID == rootTabID)
    }

    @Test("new tab from a child pane is top-level")
    func newTabFromChildIsTopLevel() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let rootAreaID = state.focusedAreaID[key]!

        _ = WorkspaceReducer.reduce(
            action: .splitArea(.init(
                projectID: projectID,
                areaID: rootAreaID,
                direction: .horizontal,
                position: .second
            )),
            state: &state
        )
        let childAreaID = state.focusedAreaID[key]!
        let effects = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: childAreaID),
            state: &state
        )
        let newTab = state.workspaceRoots[key]!.locateTab(id: effects.createdTabID!)!.tab

        #expect(newTab.parentTabID == nil)
        #expect(state.topLevelTabOrder[key]?.contains(newTab.id) == true)
    }

    @Test("pane movement rejects a target owned by another top-level tab")
    func paneMovementRejectsCrossParentTarget() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let rootAreaID = state.focusedAreaID[key]!
        let rootArea = state.workspaceRoots[key]!.findArea(id: rootAreaID)!
        let otherRootArea = TabArea(projectPath: testPath)
        let rootTabID = rootArea.tabs[0].id
        let otherRootID = otherRootArea.tabs[0].id
        state.workspaceRoots[key] = .split(SplitBranch(
            direction: .horizontal,
            first: .tabArea(rootArea),
            second: .tabArea(otherRootArea)
        ))

        _ = WorkspaceReducer.reduce(
            action: .moveTab(
                projectID: projectID,
                request: .toArea(
                    tabID: rootTabID,
                    sourceAreaID: rootAreaID,
                    destinationAreaID: otherRootArea.id
                )
            ),
            state: &state
        )

        #expect(state.workspaceRoots[key]!.findArea(id: rootAreaID)?.tabs.contains { $0.id == rootTabID } == true)
        #expect(state.workspaceRoots[key]!.findArea(id: otherRootArea.id)?.tabs.contains { $0.id == otherRootID } == true)
    }

    @Test("docking a top-level tab preserves both parent child layouts")
    func dockingTopLevelTabPreservesChildLayouts() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let rootAreaID = state.focusedAreaID[key]!
        let firstTabID = state.workspaceRoots[key]!.findArea(id: rootAreaID)!.activeTabID!
        let createEffects = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: rootAreaID),
            state: &state
        )
        let secondTabID = createEffects.createdTabID!

        _ = WorkspaceReducer.reduce(
            action: .selectTab(projectID: projectID, areaID: rootAreaID, tabID: firstTabID),
            state: &state
        )
        _ = WorkspaceReducer.reduce(
            action: .splitArea(.init(
                projectID: projectID,
                areaID: rootAreaID,
                direction: .horizontal,
                position: .second
            )),
            state: &state
        )

        _ = WorkspaceReducer.reduce(
            action: .selectTab(projectID: projectID, areaID: rootAreaID, tabID: secondTabID),
            state: &state
        )
        _ = WorkspaceReducer.reduce(
            action: .splitArea(.init(
                projectID: projectID,
                areaID: rootAreaID,
                direction: .vertical,
                position: .second
            )),
            state: &state
        )

        let root = state.workspaceRoots[key]!
        let tabOwnershipBefore = Dictionary(uniqueKeysWithValues: root.allTabs().map { ($0.id, $0.parentTabID) })
        let paneIDsBefore = Set(root.allTabs().compactMap { $0.content.pane?.id })
        let firstPaneIDsBefore = Set(
            root.visibleLayout(forTopLevelTabID: firstTabID)?.allPanes().compactMap { $0.tab.content.pane?.id } ?? []
        )
        let secondPaneIDsBefore = Set(
            root.visibleLayout(forTopLevelTabID: secondTabID)?.allPanes().compactMap { $0.tab.content.pane?.id } ?? []
        )
        let sourceGroupID = state.topLevelTabLayouts[key]!.allGroups()[0].id

        let effects = WorkspaceReducer.reduce(
            action: .moveTopLevelTab(
                projectID: projectID,
                request: .toNewSplit(
                    tabID: secondTabID,
                    sourceGroupID: sourceGroupID,
                    targetGroupID: sourceGroupID,
                    split: SplitPlacement(direction: .horizontal, position: .second)
                )
            ),
            state: &state
        )

        guard case let .split(branch) = state.topLevelTabLayouts[key] else {
            Issue.record("Expected an outer split")
            return
        }
        #expect(branch.direction == .horizontal)
        #expect(branch.first.allGroups()[0].tabIDs == [firstTabID])
        #expect(branch.second.allGroups()[0].tabIDs == [secondTabID])
        #expect(state.workspaceRoots[key]!.allTabs().count == tabOwnershipBefore.count)
        #expect(
            Dictionary(uniqueKeysWithValues: state.workspaceRoots[key]!.allTabs().map { ($0.id, $0.parentTabID) })
                == tabOwnershipBefore
        )
        #expect(Set(state.workspaceRoots[key]!.allTabs().compactMap { $0.content.pane?.id }) == paneIDsBefore)
        #expect(
            Set(
                state.workspaceRoots[key]!.visibleLayout(forTopLevelTabID: firstTabID)?
                    .allPanes().compactMap { $0.tab.content.pane?.id } ?? []
            ) == firstPaneIDsBefore
        )
        #expect(
            Set(
                state.workspaceRoots[key]!.visibleLayout(forTopLevelTabID: secondTabID)?
                    .allPanes().compactMap { $0.tab.content.pane?.id } ?? []
            ) == secondPaneIDsBefore
        )
        #expect(effects.paneIDsToRemove.isEmpty)
        #expect(effects.deferredAreaCollapses.isEmpty)
    }

    @Test("selecting child panes activates their docked parent groups")
    func selectingChildPanesActivatesDockedParentGroups() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let rootAreaID = state.focusedAreaID[key]!
        let firstTabID = state.workspaceRoots[key]!.findArea(id: rootAreaID)!.activeTabID!
        let secondTabID = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: rootAreaID),
            state: &state
        ).createdTabID!

        _ = WorkspaceReducer.reduce(
            action: .selectTab(projectID: projectID, areaID: rootAreaID, tabID: firstTabID),
            state: &state
        )
        _ = WorkspaceReducer.reduce(
            action: .splitArea(.init(
                projectID: projectID,
                areaID: rootAreaID,
                direction: .horizontal,
                position: .second
            )),
            state: &state
        )
        let firstChildAreaID = state.focusedAreaID[key]!
        let firstChildTabID = state.workspaceRoots[key]!.findArea(id: firstChildAreaID)!.activeTabID!

        _ = WorkspaceReducer.reduce(
            action: .selectTab(projectID: projectID, areaID: rootAreaID, tabID: secondTabID),
            state: &state
        )
        _ = WorkspaceReducer.reduce(
            action: .splitArea(.init(
                projectID: projectID,
                areaID: rootAreaID,
                direction: .vertical,
                position: .second
            )),
            state: &state
        )
        let secondChildAreaID = state.focusedAreaID[key]!
        let secondChildTabID = state.workspaceRoots[key]!.findArea(id: secondChildAreaID)!.activeTabID!
        let initialGroupID = state.topLevelTabLayouts[key]!.allGroups()[0].id

        _ = WorkspaceReducer.reduce(
            action: .moveTopLevelTab(
                projectID: projectID,
                request: .toNewSplit(
                    tabID: secondTabID,
                    sourceGroupID: initialGroupID,
                    targetGroupID: initialGroupID,
                    split: SplitPlacement(direction: .horizontal, position: .second)
                )
            ),
            state: &state
        )
        _ = WorkspaceReducer.reduce(
            action: .selectTab(
                projectID: projectID,
                areaID: firstChildAreaID,
                tabID: firstChildTabID
            ),
            state: &state
        )

        #expect(state.focusedAreaID[key] == firstChildAreaID)
        #expect(state.workspaceRoots[key]!.findArea(id: firstChildAreaID)?.activeTabID == firstChildTabID)
        #expect(state.topLevelTabLayouts[key]!.group(containingTabID: firstTabID)?.activeTabID == firstTabID)

        _ = WorkspaceReducer.reduce(
            action: .selectTab(
                projectID: projectID,
                areaID: secondChildAreaID,
                tabID: secondChildTabID
            ),
            state: &state
        )

        #expect(state.focusedAreaID[key] == secondChildAreaID)
        #expect(state.workspaceRoots[key]!.findArea(id: secondChildAreaID)?.activeTabID == secondChildTabID)
        #expect(state.topLevelTabLayouts[key]!.group(containingTabID: secondTabID)?.activeTabID == secondTabID)
    }

    @Test("center docking merges top-level groups and collapses the empty group")
    func centerDockingMergesTopLevelGroups() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let areaID = state.focusedAreaID[key]!
        let firstTabID = state.workspaceRoots[key]!.findArea(id: areaID)!.activeTabID!
        let createEffects = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: areaID),
            state: &state
        )
        let secondTabID = createEffects.createdTabID!
        let initialGroupID = state.topLevelTabLayouts[key]!.allGroups()[0].id

        _ = WorkspaceReducer.reduce(
            action: .moveTopLevelTab(
                projectID: projectID,
                request: .toNewSplit(
                    tabID: secondTabID,
                    sourceGroupID: initialGroupID,
                    targetGroupID: initialGroupID,
                    split: SplitPlacement(direction: .horizontal, position: .second)
                )
            ),
            state: &state
        )

        let groups = state.topLevelTabLayouts[key]!.allGroups()
        let firstGroupID = groups.first(where: { $0.tabIDs.contains(firstTabID) })!.id
        let secondGroupID = groups.first(where: { $0.tabIDs.contains(secondTabID) })!.id
        _ = WorkspaceReducer.reduce(
            action: .moveTopLevelTab(
                projectID: projectID,
                request: .toGroup(
                    tabID: secondTabID,
                    sourceGroupID: secondGroupID,
                    destinationGroupID: firstGroupID
                )
            ),
            state: &state
        )

        guard case let .group(group) = state.topLevelTabLayouts[key] else {
            Issue.record("Expected merged top-level group")
            return
        }
        #expect(group.tabIDs == [firstTabID, secondTabID])
        #expect(group.activeTabID == secondTabID)
        #expect(state.workspaceRoots[key]!.allTabs().count == 2)
    }

    @Test("center docking keeps pinned top-level tabs before unpinned tabs")
    func centerDockingPreservesPinnedFirstOrdering() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let areaID = state.focusedAreaID[key]!
        let firstTabID = state.workspaceRoots[key]!.findArea(id: areaID)!.activeTabID!
        let secondTabID = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: areaID),
            state: &state
        ).createdTabID!
        let secondTab = state.workspaceRoots[key]!.locateTab(id: secondTabID)!.tab
        secondTab.isPinned = true
        state.topLevelTabOrder[key] = [secondTabID, firstTabID]
        let initialGroup = state.topLevelTabLayouts[key]!.allGroups()[0]
        initialGroup.tabIDs = [secondTabID, firstTabID]

        _ = WorkspaceReducer.reduce(
            action: .moveTopLevelTab(
                projectID: projectID,
                request: .toNewSplit(
                    tabID: secondTabID,
                    sourceGroupID: initialGroup.id,
                    targetGroupID: initialGroup.id,
                    split: SplitPlacement(direction: .horizontal, position: .second)
                )
            ),
            state: &state
        )

        let pinnedGroupID = state.topLevelTabLayouts[key]!.group(containingTabID: secondTabID)!.id
        let unpinnedGroupID = state.topLevelTabLayouts[key]!.group(containingTabID: firstTabID)!.id
        _ = WorkspaceReducer.reduce(
            action: .moveTopLevelTab(
                projectID: projectID,
                request: .toGroup(
                    tabID: secondTabID,
                    sourceGroupID: pinnedGroupID,
                    destinationGroupID: unpinnedGroupID
                )
            ),
            state: &state
        )

        let mergedGroup = state.topLevelTabLayouts[key]!.group(id: unpinnedGroupID)
        #expect(mergedGroup?.tabIDs == [secondTabID, firstTabID])
    }

    @Test("closing an inactive docked tab preserves the active group")
    func closingInactiveDockedTabPreservesActiveGroup() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let areaID = state.focusedAreaID[key]!
        let firstTabID = state.workspaceRoots[key]!.findArea(id: areaID)!.activeTabID!
        let secondTabID = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: areaID),
            state: &state
        ).createdTabID!
        let thirdTabID = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: areaID),
            state: &state
        ).createdTabID!
        let initialGroupID = state.topLevelTabLayouts[key]!.allGroups()[0].id

        _ = WorkspaceReducer.reduce(
            action: .moveTopLevelTab(
                projectID: projectID,
                request: .toNewSplit(
                    tabID: firstTabID,
                    sourceGroupID: initialGroupID,
                    targetGroupID: initialGroupID,
                    split: SplitPlacement(direction: .horizontal, position: .first)
                )
            ),
            state: &state
        )
        _ = WorkspaceReducer.reduce(
            action: .closeTab(projectID: projectID, areaID: areaID, tabID: thirdTabID),
            state: &state
        )

        #expect(state.focusedAreaID[key] == areaID)
        #expect(state.workspaceRoots[key]!.findArea(id: areaID)?.activeTabID == firstTabID)
        #expect(state.topLevelTabLayouts[key]!.group(containingTabID: secondTabID)?.activeTabID == secondTabID)
    }

    @Test("directional focus moves between docked top-level groups")
    func directionalFocusMovesBetweenDockedGroups() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let areaID = state.focusedAreaID[key]!
        let firstTabID = state.workspaceRoots[key]!.findArea(id: areaID)!.activeTabID!
        let secondTabID = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: areaID),
            state: &state
        ).createdTabID!
        let groupID = state.topLevelTabLayouts[key]!.allGroups()[0].id

        _ = WorkspaceReducer.reduce(
            action: .moveTopLevelTab(
                projectID: projectID,
                request: .toNewSplit(
                    tabID: secondTabID,
                    sourceGroupID: groupID,
                    targetGroupID: groupID,
                    split: SplitPlacement(direction: .horizontal, position: .second)
                )
            ),
            state: &state
        )
        _ = WorkspaceReducer.reduce(
            action: .selectTab(projectID: projectID, areaID: areaID, tabID: firstTabID),
            state: &state
        )
        _ = WorkspaceReducer.reduce(
            action: .focusPaneRight(projectID: projectID),
            state: &state
        )

        #expect(state.workspaceRoots[key]!.findArea(id: areaID)?.activeTabID == secondTabID)
        #expect(state.topLevelTabLayouts[key]!.group(containingTabID: secondTabID)?.activeTabID == secondTabID)
    }

    @Test("closing an active docked tab prefers a sibling in the same group")
    func closingActiveDockedTabPrefersSameGroup() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let areaID = state.focusedAreaID[key]!
        let firstTabID = state.workspaceRoots[key]!.findArea(id: areaID)!.activeTabID!
        let secondTabID = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: areaID),
            state: &state
        ).createdTabID!
        let thirdTabID = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: areaID),
            state: &state
        ).createdTabID!
        let initialGroup = state.topLevelTabLayouts[key]!.allGroups()[0]
        initialGroup.tabIDs = [thirdTabID, firstTabID, secondTabID]
        state.topLevelTabOrder[key] = initialGroup.tabIDs

        _ = WorkspaceReducer.reduce(
            action: .moveTopLevelTab(
                projectID: projectID,
                request: .toNewSplit(
                    tabID: secondTabID,
                    sourceGroupID: initialGroup.id,
                    targetGroupID: initialGroup.id,
                    split: SplitPlacement(direction: .horizontal, position: .second)
                )
            ),
            state: &state
        )
        _ = WorkspaceReducer.reduce(
            action: .selectTab(projectID: projectID, areaID: areaID, tabID: firstTabID),
            state: &state
        )
        _ = WorkspaceReducer.reduce(
            action: .closeTab(projectID: projectID, areaID: areaID, tabID: firstTabID),
            state: &state
        )

        #expect(state.workspaceRoots[key]!.findArea(id: areaID)?.activeTabID == thirdTabID)
        #expect(state.topLevelTabLayouts[key]!.group(containingTabID: thirdTabID)?.activeTabID == thirdTabID)
    }

    @Test("new top-level tabs stay in the selected docked group")
    func newTopLevelTabsStayInSelectedDockedGroup() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let areaID = state.focusedAreaID[key]!
        let firstTabID = state.workspaceRoots[key]!.findArea(id: areaID)!.activeTabID!
        let secondTabID = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: areaID),
            state: &state
        ).createdTabID!
        let initialGroupID = state.topLevelTabLayouts[key]!.allGroups()[0].id

        _ = WorkspaceReducer.reduce(
            action: .moveTopLevelTab(
                projectID: projectID,
                request: .toNewSplit(
                    tabID: secondTabID,
                    sourceGroupID: initialGroupID,
                    targetGroupID: initialGroupID,
                    split: SplitPlacement(direction: .horizontal, position: .second)
                )
            ),
            state: &state
        )

        let secondGroup = state.topLevelTabLayouts[key]!.group(containingTabID: secondTabID)!
        _ = WorkspaceReducer.reduce(
            action: .selectTab(projectID: projectID, areaID: areaID, tabID: secondTabID),
            state: &state
        )
        let thirdTabID = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: areaID),
            state: &state
        ).createdTabID!

        #expect(secondGroup.tabIDs == [secondTabID, thirdTabID])
        #expect(state.topLevelTabLayouts[key]!.group(containingTabID: firstTabID)?.tabIDs == [firstTabID])
    }

    @Test("repeated singleton group docking preserves identities and pane ownership")
    func repeatedSingletonGroupDockingPreservesIdentityAndPaneOwnership() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let rootAreaID = state.focusedAreaID[key]!
        let firstTabID = state.workspaceRoots[key]!.findArea(id: rootAreaID)!.activeTabID!
        let secondTabID = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: rootAreaID),
            state: &state
        ).createdTabID!
        let thirdTabID = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: rootAreaID),
            state: &state
        ).createdTabID!

        _ = WorkspaceReducer.reduce(
            action: .selectTab(projectID: projectID, areaID: rootAreaID, tabID: firstTabID),
            state: &state
        )
        _ = WorkspaceReducer.reduce(
            action: .splitArea(.init(
                projectID: projectID,
                areaID: rootAreaID,
                direction: .vertical,
                position: .second
            )),
            state: &state
        )

        let initialGroupID = state.topLevelTabLayouts[key]!.allGroups()[0].id
        _ = WorkspaceReducer.reduce(
            action: .moveTopLevelTab(
                projectID: projectID,
                request: .toNewSplit(
                    tabID: secondTabID,
                    sourceGroupID: initialGroupID,
                    targetGroupID: initialGroupID,
                    split: SplitPlacement(direction: .horizontal, position: .second)
                )
            ),
            state: &state
        )
        let secondGroupID = state.topLevelTabLayouts[key]!.group(containingTabID: secondTabID)!.id
        _ = WorkspaceReducer.reduce(
            action: .moveTopLevelTab(
                projectID: projectID,
                request: .toNewSplit(
                    tabID: thirdTabID,
                    sourceGroupID: initialGroupID,
                    targetGroupID: secondGroupID,
                    split: SplitPlacement(direction: .vertical, position: .first)
                )
            ),
            state: &state
        )

        let firstGroupID = state.topLevelTabLayouts[key]!.group(containingTabID: firstTabID)!.id
        let thirdGroupID = state.topLevelTabLayouts[key]!.group(containingTabID: thirdTabID)!.id
        let expectedGroupIDs = [
            firstTabID: firstGroupID,
            secondTabID: secondGroupID,
            thirdTabID: thirdGroupID,
        ]
        let expectedPaneIDs = Dictionary(uniqueKeysWithValues: expectedGroupIDs.keys.map { tabID in
            (
                tabID,
                Set(
                    state.workspaceRoots[key]!.visibleLayout(forTopLevelTabID: tabID)?
                        .allPanes().compactMap { $0.tab.content.pane?.id } ?? []
                )
            )
        })
        let moves = [
            (
                tabID: secondTabID,
                sourceGroupID: secondGroupID,
                targetGroupID: thirdGroupID,
                split: SplitPlacement(direction: .horizontal, position: .first)
            ),
            (
                tabID: firstTabID,
                sourceGroupID: firstGroupID,
                targetGroupID: secondGroupID,
                split: SplitPlacement(direction: .vertical, position: .second)
            ),
            (
                tabID: thirdTabID,
                sourceGroupID: thirdGroupID,
                targetGroupID: firstGroupID,
                split: SplitPlacement(direction: .horizontal, position: .second)
            ),
        ]

        for _ in 0 ..< 3 {
            for move in moves {
                _ = WorkspaceReducer.reduce(
                    action: .moveTopLevelTab(
                        projectID: projectID,
                        request: .toNewSplit(
                            tabID: move.tabID,
                            sourceGroupID: move.sourceGroupID,
                            targetGroupID: move.targetGroupID,
                            split: move.split
                        )
                    ),
                    state: &state
                )

                let layout = state.topLevelTabLayouts[key]!
                let flattenedTabIDs = layout.flattenedTabIDs()
                #expect(flattenedTabIDs.count == expectedGroupIDs.count)
                #expect(Set(flattenedTabIDs) == Set(expectedGroupIDs.keys))
                for (tabID, groupID) in expectedGroupIDs {
                    let group = layout.group(containingTabID: tabID)
                    #expect(group?.id == groupID)
                    #expect(group?.activeTabID == tabID)
                }
                for (tabID, paneIDs) in expectedPaneIDs {
                    let currentPaneIDs = Set(
                        state.workspaceRoots[key]!.visibleLayout(forTopLevelTabID: tabID)?
                            .allPanes().compactMap { $0.tab.content.pane?.id } ?? []
                    )
                    #expect(currentPaneIDs == paneIDs)
                }
                let projectedPaneIDs = expectedPaneIDs.values
                #expect(
                    projectedPaneIDs.reduce(into: Set<UUID>()) { result, paneIDs in
                        result.formUnion(paneIDs)
                    }.count == projectedPaneIDs.reduce(0) { $0 + $1.count }
                )
            }
        }
    }

    @Test("merged tab receives one stable group identity when split out again")
    func mergedTabReceivesStableIdentityWhenExtractedAgain() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let areaID = state.focusedAreaID[key]!
        let firstTabID = state.workspaceRoots[key]!.findArea(id: areaID)!.activeTabID!
        let secondTabID = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: areaID),
            state: &state
        ).createdTabID!
        let retainedGroupID = state.topLevelTabLayouts[key]!.allGroups()[0].id

        _ = WorkspaceReducer.reduce(
            action: .moveTopLevelTab(
                projectID: projectID,
                request: .toNewSplit(
                    tabID: secondTabID,
                    sourceGroupID: retainedGroupID,
                    targetGroupID: retainedGroupID,
                    split: SplitPlacement(direction: .horizontal, position: .second)
                )
            ),
            state: &state
        )
        let mergedGroupID = state.topLevelTabLayouts[key]!.group(containingTabID: secondTabID)!.id
        _ = WorkspaceReducer.reduce(
            action: .moveTopLevelTab(
                projectID: projectID,
                request: .toGroup(
                    tabID: secondTabID,
                    sourceGroupID: mergedGroupID,
                    destinationGroupID: retainedGroupID
                )
            ),
            state: &state
        )

        #expect(state.topLevelTabLayouts[key]!.group(id: mergedGroupID) == nil)
        #expect(state.topLevelTabLayouts[key]!.group(id: retainedGroupID)?.tabIDs == [firstTabID, secondTabID])

        let paneIDsBefore = Set(state.workspaceRoots[key]!.allTabs().compactMap { $0.content.pane?.id })
        _ = WorkspaceReducer.reduce(
            action: .moveTopLevelTab(
                projectID: projectID,
                request: .toNewSplit(
                    tabID: secondTabID,
                    sourceGroupID: retainedGroupID,
                    targetGroupID: retainedGroupID,
                    split: SplitPlacement(direction: .vertical, position: .first)
                )
            ),
            state: &state
        )
        let extractedGroupID = state.topLevelTabLayouts[key]!.group(containingTabID: secondTabID)!.id

        #expect(extractedGroupID != mergedGroupID)
        #expect(state.topLevelTabLayouts[key]!.group(containingTabID: firstTabID)?.id == retainedGroupID)

        let placements = [
            SplitPlacement(direction: .horizontal, position: .second),
            SplitPlacement(direction: .vertical, position: .first),
            SplitPlacement(direction: .vertical, position: .second),
        ]
        for placement in placements {
            _ = WorkspaceReducer.reduce(
                action: .moveTopLevelTab(
                    projectID: projectID,
                    request: .toNewSplit(
                        tabID: secondTabID,
                        sourceGroupID: extractedGroupID,
                        targetGroupID: retainedGroupID,
                        split: placement
                    )
                ),
                state: &state
            )

            let layout = state.topLevelTabLayouts[key]!
            #expect(layout.group(containingTabID: firstTabID)?.id == retainedGroupID)
            #expect(layout.group(containingTabID: secondTabID)?.id == extractedGroupID)
            #expect(Set(layout.flattenedTabIDs()) == Set([firstTabID, secondTabID]))
            #expect(Set(state.workspaceRoots[key]!.allTabs().compactMap { $0.content.pane?.id }) == paneIDsBefore)
            guard case let .split(branch) = layout else {
                Issue.record("Expected two docked groups")
                return
            }
            #expect(branch.direction == placement.direction)
            let firstGroupID = branch.first.allGroups()[0].id
            let secondGroupID = branch.second.allGroups()[0].id
            #expect(firstGroupID == (placement.position == .first ? extractedGroupID : retainedGroupID))
            #expect(secondGroupID == (placement.position == .first ? retainedGroupID : extractedGroupID))
        }
    }

    @Test("directional pane movement swaps the nearest owned panes")
    func directionalPaneMovementSwapsNearestOwnedPanes() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let leftAreaID = state.focusedAreaID[key]!
        let leftTabID = state.workspaceRoots[key]!.findArea(id: leftAreaID)!.activeTabID!

        _ = WorkspaceReducer.reduce(
            action: .splitArea(.init(
                projectID: projectID,
                areaID: leftAreaID,
                direction: .horizontal,
                position: .second
            )),
            state: &state
        )
        let rightAreaID = state.focusedAreaID[key]!
        let rightTabID = state.workspaceRoots[key]!.findArea(id: rightAreaID)!.activeTabID!
        _ = WorkspaceReducer.reduce(
            action: .focusArea(projectID: projectID, areaID: leftAreaID),
            state: &state
        )

        _ = WorkspaceReducer.reduce(
            action: .movePaneRight(projectID: projectID),
            state: &state
        )

        #expect(state.workspaceRoots[key]!.findArea(id: leftAreaID)?.activeTabID == rightTabID)
        #expect(state.workspaceRoots[key]!.findArea(id: rightAreaID)?.activeTabID == leftTabID)
        #expect(state.focusedAreaID[key] == rightAreaID)
    }
}
