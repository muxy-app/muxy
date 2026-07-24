import Foundation
import Testing

@testable import Muxy

@Suite("TabPaneReducer")
@MainActor
struct TabPaneReducerTests {
    private let testPath = "/tmp/test"

    private func makeState(
        projectID: UUID,
        worktreeID: UUID
    ) -> WorkspaceState {
        var state = WorkspaceState(
            activeProjectID: projectID,
            activeWorktreeID: [projectID: worktreeID],
            workspaceRoots: [:],
            focusedAreaID: [:],
            focusHistory: [:]
        )
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = TabArea(projectPath: testPath)
        state.workspaceRoots[key] = .tabArea(area)
        state.focusedAreaID[key] = area.id
        return state
    }

    @Test("splitTabPane creates second pane within tab")
    func splitTabPaneCreatesSecondPane() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = state.workspaceRoots[key]!.allAreas().first!
        let tabID = area.activeTabID!

        let newPaneID = TabPaneReducer.splitTabPane(
            projectID: projectID, areaID: area.id, tabID: tabID,
            direction: .horizontal, state: &state
        )

        #expect(newPaneID != nil)
        let tab = area.tabs.first { $0.id == tabID }
        #expect(tab?.internalPanes != nil)
        let panes = tab!.internalPanes!.allPanes()
        #expect(panes.count == 2)
        #expect(tab?.focusedPaneID == newPaneID)
    }

    @Test("splitTabPane sets focus to new pane")
    func splitTabPaneFocusesNewPane() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = state.workspaceRoots[key]!.allAreas().first!
        let tabID = area.activeTabID!

        let newPaneID = TabPaneReducer.splitTabPane(
            projectID: projectID, areaID: area.id, tabID: tabID,
            direction: .horizontal, state: &state
        )

        let tab = area.tabs.first { $0.id == tabID }
        #expect(tab?.focusedPaneID == newPaneID)
        #expect(tab?.focusedPaneID != area.tabs.first?.content.pane?.id)
    }

    @Test("splitTabPane twice creates three panes")
    func splitTabPaneTwice() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = state.workspaceRoots[key]!.allAreas().first!
        let tabID = area.activeTabID!

        let firstPaneID = TabPaneReducer.splitTabPane(
            projectID: projectID, areaID: area.id, tabID: tabID,
            direction: .horizontal, state: &state
        )

        let secondPaneID = TabPaneReducer.splitTabPane(
            projectID: projectID, areaID: area.id, tabID: tabID,
            direction: .vertical, state: &state
        )

        let tab = area.tabs.first { $0.id == tabID }
        let panes = tab!.internalPanes!.allPanes()
        #expect(panes.count == 3)
        #expect(tab?.focusedPaneID == secondPaneID)
    }

    @Test("closing one of three panes preserves the remaining pane identities")
    func closeOneOfThreePanesPreservesRemainingIdentities() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = state.workspaceRoots[key]!.allAreas().first!
        let tabID = area.activeTabID!
        let firstNewPaneID = TabPaneReducer.splitTabPane(
            projectID: projectID,
            areaID: area.id,
            tabID: tabID,
            direction: .horizontal,
            state: &state
        )!
        let secondNewPaneID = TabPaneReducer.splitTabPane(
            projectID: projectID,
            areaID: area.id,
            tabID: tabID,
            direction: .vertical,
            state: &state
        )!
        let tab = area.tabs.first { $0.id == tabID }!
        let paneIDsBeforeClose = Set(tab.internalPanes!.allPanes().map(\.id))

        let removedPaneID = TabPaneReducer.closeInternalPane(
            projectID: projectID,
            areaID: area.id,
            tabID: tabID,
            paneID: firstNewPaneID,
            state: &state
        )

        #expect(removedPaneID == firstNewPaneID)
        #expect(Set(tab.internalPanes!.allPanes().map(\.id)) == paneIDsBeforeClose.subtracting([firstNewPaneID]))
        #expect(tab.focusedPaneID == secondNewPaneID)
    }

    @Test("splitTabPane on non-terminal content does nothing")
    func splitTabPaneNonTerminal() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = state.workspaceRoots[key]!.allAreas().first!

        let browserTabID = area.createBrowserTab(url: nil)
        let tab = area.tabs.first { $0.id == browserTabID }
        #expect(tab != nil)

        let newPaneID = TabPaneReducer.splitTabPane(
            projectID: projectID, areaID: area.id, tabID: browserTabID,
            direction: .horizontal, state: &state
        )

        #expect(newPaneID == nil)
        #expect(tab?.internalPanes == nil)
    }

    @Test("closeInternalPane removes pane and collapses if last")
    func closeInternalPane() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = state.workspaceRoots[key]!.allAreas().first!
        let tabID = area.activeTabID!

        let newPaneID = TabPaneReducer.splitTabPane(
            projectID: projectID, areaID: area.id, tabID: tabID,
            direction: .horizontal, state: &state
        )

        let tab = area.tabs.first { $0.id == tabID }
        #expect(tab?.internalPanes != nil)

        let removed = TabPaneReducer.closeInternalPane(
            projectID: projectID, areaID: area.id, tabID: tabID,
            paneID: newPaneID!, state: &state
        )

        #expect(removed != nil)
        #expect(removed == newPaneID)
        #expect(tab?.internalPanes == nil)
        #expect(tab?.focusedPaneID == nil)
    }

    @Test("closing the original pane preserves the surviving session")
    func closingOriginalPanePreservesSurvivingSession() {
        let projectID = UUID()
        let worktreeID = UUID()
        var state = makeState(projectID: projectID, worktreeID: worktreeID)
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = state.workspaceRoots[key]!.allAreas().first!
        let tabID = area.activeTabID!
        let originalPaneID = area.activeTab!.content.pane!.id
        let survivingPaneID = TabPaneReducer.splitTabPane(
            projectID: projectID,
            areaID: area.id,
            tabID: tabID,
            direction: .horizontal,
            state: &state
        )!

        _ = TabPaneReducer.closeInternalPane(
            projectID: projectID,
            areaID: area.id,
            tabID: tabID,
            paneID: originalPaneID,
            state: &state
        )

        let tab = area.tabs.first { $0.id == tabID }!
        #expect(tab.internalPanes?.allPanes().map(\.id) == [survivingPaneID])
        #expect(tab.focusedPaneID == survivingPaneID)
        #expect(tab.displayPane?.id == survivingPaneID)
    }
}
