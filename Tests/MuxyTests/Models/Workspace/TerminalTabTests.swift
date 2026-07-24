import Testing

@testable import Muxy

@Suite("TerminalTab")
@MainActor
struct TerminalTabInternalPanesTests {
    private let testPath = "/tmp/test"

    @Test("new tab has no internal panes")
    func newTabNoInternalPanes() {
        let tab = TerminalTab(pane: TerminalPaneState(projectPath: testPath))
        #expect(tab.internalPanes == nil)
        #expect(tab.focusedPaneID == nil)
    }

    @Test("tab with internal panes reports multiple panes")
    func tabWithInternalPanes() {
        let tab = TerminalTab(pane: TerminalPaneState(projectPath: testPath))
        let p1 = TerminalPaneState(projectPath: testPath)
        let p2 = TerminalPaneState(projectPath: testPath)
        tab.internalPanes = .split(InternalBranch(
            direction: .horizontal, first: .pane(p1), second: .pane(p2)
        ))
        tab.focusedPaneID = p1.id
        #expect(tab.internalPanes != nil)
        let panes = tab.internalPanes!.allPanes()
        #expect(panes.count == 2)
        #expect(tab.focusedPaneID == p1.id)
    }

    @Test("setting internalPanes to nil restores single pane behavior")
    func clearingInternalPanes() {
        let tab = TerminalTab(pane: TerminalPaneState(projectPath: testPath))
        let p1 = TerminalPaneState(projectPath: testPath)
        let p2 = TerminalPaneState(projectPath: testPath)
        tab.internalPanes = .split(InternalBranch(
            direction: .horizontal, first: .pane(p1), second: .pane(p2)
        ))
        tab.focusedPaneID = p1.id
        tab.internalPanes = nil
        tab.focusedPaneID = nil
        #expect(tab.internalPanes == nil)
        #expect(tab.focusedPaneID == nil)
    }

    @Test("tab title follows the focused internal pane process")
    func titleFollowsFocusedInternalPaneProcess() {
        let rootPane = TerminalPaneState(projectPath: testPath, title: "root")
        let firstPane = TerminalPaneState(projectPath: testPath, title: "first")
        let secondPane = TerminalPaneState(projectPath: testPath, title: "second")
        let tab = TerminalTab(pane: rootPane)
        tab.internalPanes = .split(InternalBranch(
            direction: .horizontal,
            first: .pane(firstPane),
            second: .pane(secondPane)
        ))
        firstPane.setForegroundProcessName("vim")
        secondPane.setForegroundProcessName("swift")

        tab.focusedPaneID = firstPane.id
        #expect(tab.title == "vim")
        #expect(tab.displayPane?.id == firstPane.id)

        tab.focusedPaneID = secondPane.id
        #expect(tab.title == "swift")
        #expect(tab.displayPane?.id == secondPane.id)
    }

    @Test("custom tab title overrides focused internal pane process")
    func customTitleOverridesFocusedInternalPaneProcess() {
        let rootPane = TerminalPaneState(projectPath: testPath)
        let innerPane = TerminalPaneState(projectPath: testPath)
        innerPane.setForegroundProcessName("vim")
        let tab = TerminalTab(pane: rootPane)
        tab.internalPanes = .pane(innerPane)
        tab.focusedPaneID = innerPane.id
        tab.customTitle = "Editor"

        #expect(tab.title == "Editor")
    }
}
