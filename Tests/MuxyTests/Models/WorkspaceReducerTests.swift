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
        _ = WorkspaceReducer.reduce(action: action, state: &state)

        let area = focusedArea(in: state, projectID: projectID)
        #expect(area?.tabs.count == 2)
    }

    @Test("createVCSTab adds VCS tab")
    func createVCSTab() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)

        let action = AppState.Action.createVCSTab(projectID: projectID, areaID: nil)
        _ = WorkspaceReducer.reduce(action: action, state: &state)

        let area = focusedArea(in: state, projectID: projectID)
        #expect(area?.activeTab?.kind == .vcs)
    }

    @Test("createEditorTab adds editor tab")
    func createEditorTab() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)

        let action = AppState.Action.createEditorTab(
            projectID: projectID, areaID: nil, filePath: "/tmp/test/file.swift", suppressInitialFocus: false
        )
        _ = WorkspaceReducer.reduce(action: action, state: &state)

        let area = focusedArea(in: state, projectID: projectID)
        #expect(area?.activeTab?.kind == .editor)
    }

    @Test("createExternalEditorTab adds terminal editor tab")
    func createExternalEditorTab() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)

        let action = AppState.Action.createExternalEditorTab(
            projectID: projectID,
            areaID: nil,
            filePath: "/tmp/test/file.swift",
            command: "vim"
        )
        _ = WorkspaceReducer.reduce(action: action, state: &state)

        let area = focusedArea(in: state, projectID: projectID)
        #expect(area?.activeTab?.kind == .terminal)
        #expect(area?.activeTab?.content.pane?.externalEditorFilePath == "/tmp/test/file.swift")
    }

    @Test("closeTab removes tab and populates paneIDsToRemove")
    func closeTab() {
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

        let effects = WorkspaceReducer.reduce(
            action: .closeTab(projectID: projectID, areaID: areaID, tabID: firstTabID),
            state: &state
        )
        #expect(area.tabs.count == 1)
        #expect(!effects.paneIDsToRemove.isEmpty)
    }

    @Test("closeTab last tab in multi-area closes area instead")
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

        #expect(!state.workspaceRoots[key]!.containsArea(id: firstAreaID))
        #expect(!effects.paneIDsToRemove.isEmpty)
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
        #expect(effects.projectIDsToRemove.contains(projectID))
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

        _ = WorkspaceReducer.reduce(
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

    @Test("cycleNextTabAcrossPanes walks tabs in focused pane before next pane")
    func cycleNextTabAcrossPanesWalksTabsBeforeNextPane() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let firstAreaID = state.focusedAreaID[key]!

        _ = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: firstAreaID),
            state: &state
        )
        let firstAreaTabs = area(in: state, key: key, areaID: firstAreaID)!.tabs
        let firstTabID = firstAreaTabs[0].id
        let secondTabID = firstAreaTabs[1].id

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
            action: .selectTab(projectID: projectID, areaID: firstAreaID, tabID: firstTabID),
            state: &state
        )

        _ = WorkspaceReducer.reduce(
            action: .cycleNextTabAcrossPanes(projectID: projectID),
            state: &state
        )
        #expect(state.focusedAreaID[key] == firstAreaID)
        #expect(area(in: state, key: key, areaID: firstAreaID)?.activeTabID == secondTabID)

        _ = WorkspaceReducer.reduce(
            action: .cycleNextTabAcrossPanes(projectID: projectID),
            state: &state
        )
        #expect(state.focusedAreaID[key] == secondAreaID)
        #expect(area(in: state, key: key, areaID: secondAreaID)?.activeTabID == secondAreaTabID)
    }

    @Test("cyclePreviousTabAcrossPanes walks backward across panes")
    func cyclePreviousTabAcrossPanesWalksBackward() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let firstAreaID = state.focusedAreaID[key]!

        _ = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: firstAreaID),
            state: &state
        )
        let secondFirstAreaTabID = area(in: state, key: key, areaID: firstAreaID)!.tabs[1].id

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
        #expect(area(in: state, key: key, areaID: firstAreaID)?.activeTabID == secondFirstAreaTabID)
    }

    @Test("cycleTabAcrossPanes wraps between first and last entries")
    func cycleTabAcrossPanesWrapsBetweenFirstAndLastEntries() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let firstAreaID = state.focusedAreaID[key]!

        _ = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: firstAreaID),
            state: &state
        )
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

    @Test("moveTab toArea moves tab between areas")
    func moveTabToArea() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let firstAreaID = state.focusedAreaID[key]!

        _ = WorkspaceReducer.reduce(
            action: .createTab(projectID: projectID, areaID: nil),
            state: &state
        )

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

        _ = WorkspaceReducer.reduce(
            action: .moveTab(
                projectID: projectID,
                request: .toArea(tabID: tabToMove, sourceAreaID: firstAreaID, destinationAreaID: secondAreaID)
            ),
            state: &state
        )

        let destArea = state.workspaceRoots[key]!.findArea(id: secondAreaID)!
        #expect(destArea.tabs.contains(where: { $0.id == tabToMove }))
    }

    @Test("moveTab toArea defers collapse of empty source area")
    func moveTabToAreaDefersCollapse() {
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
        #expect(state.workspaceRoots[key]!.findArea(id: firstAreaID)!.tabs.isEmpty)
        #expect(effects.deferredAreaCollapses.contains(where: { $0.areaID == firstAreaID }))
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
            action: .selectTabByIndex(projectID: projectID, areaID: nil, index: 0),
            state: &state
        )

        let area = focusedArea(in: state, projectID: projectID)!
        #expect(area.activeTabID == area.tabs[0].id)
    }
}
