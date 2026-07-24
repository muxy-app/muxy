import Testing

@testable import Muxy

@Suite("Tab Focused internal pane rows")
struct TabFocusedInternalPaneRowTests {
    @Test("all rows before the final pane continue the tree branch")
    func nonFinalRowsContinueTreeBranch() {
        #expect(TabFocusedInternalPaneRowPosition(index: 0, count: 3) == .branch)
        #expect(TabFocusedInternalPaneRowPosition(index: 1, count: 3) == .branch)
    }

    @Test("final pane terminates the tree branch")
    func finalRowTerminatesTreeBranch() {
        #expect(TabFocusedInternalPaneRowPosition(index: 2, count: 3) == .last)
    }

    @Test("pane icon uses its detected provider with terminal fallback")
    func paneIconUsesDetectedProvider() {
        #expect(TabFocusedInternalPaneIcon(agentIconName: "claude") == .provider("claude"))
        #expect(TabFocusedInternalPaneIcon(agentIconName: nil) == .terminal)
    }

    @Test("tab status summary includes every internal pane")
    @MainActor
    func statusSummaryIncludesInternalPanes() {
        let rootPane = TerminalPaneState(projectPath: "/tmp")
        let firstPane = TerminalPaneState(projectPath: "/tmp")
        let secondPane = TerminalPaneState(projectPath: "/tmp")
        let tab = TerminalTab(pane: rootPane)
        tab.internalPanes = .split(InternalBranch(
            direction: .horizontal,
            first: .pane(firstPane),
            second: .pane(secondPane)
        ))

        #expect(Set(TabFocusedTabStatusSummary.paneIDs(in: [tab])) == [firstPane.id, secondPane.id])
    }

    @Test("waiting internal pane drives parent tab status")
    @MainActor
    func waitingInternalPaneDrivesParentStatus() {
        let rootPane = TerminalPaneState(projectPath: "/tmp")
        let firstPane = TerminalPaneState(projectPath: "/tmp")
        let secondPane = TerminalPaneState(projectPath: "/tmp")
        let tab = TerminalTab(pane: rootPane)
        tab.internalPanes = .split(InternalBranch(
            direction: .horizontal,
            first: .pane(firstPane),
            second: .pane(secondPane)
        ))

        let status = TabFocusedTabStatusSummary.agentStatus(in: [tab]) {
            $0 == secondPane.id ? .waiting : nil
        }

        #expect(status == .waiting)
    }

    @Test("working internal pane drives parent tab spinner")
    @MainActor
    func workingInternalPaneDrivesParentSpinner() {
        let rootPane = TerminalPaneState(projectPath: "/tmp")
        let innerPane = TerminalPaneState(projectPath: "/tmp")
        let tab = TerminalTab(pane: rootPane)
        tab.internalPanes = .pane(innerPane)

        let status = TabFocusedTabStatusSummary.agentStatus(in: [tab]) {
            $0 == innerPane.id ? .working : nil
        }
        let indicator = TerminalProgress.tabIndicator(progress: nil, agentStatus: status)

        #expect(status == .working)
        #expect(indicator?.kind == .indeterminate)
    }
}
