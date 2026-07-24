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
}
