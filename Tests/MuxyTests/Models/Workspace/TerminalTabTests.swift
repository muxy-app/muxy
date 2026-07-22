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
}
