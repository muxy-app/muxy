import Foundation
import Testing

@testable import Muxy

@Suite("InternalPaneNodeSnapshot")
@MainActor
struct InternalPaneNodeSnapshotTests {
    private let testPath = "/tmp/test"

    @Test("pane snapshot round-trip")
    func paneSnapshot() {
        let pane = TerminalPaneState(projectPath: testPath, title: "test-pane")
        let node = InternalPaneNode.pane(pane)
        let snapshot = InternalPaneNodeSnapshot(node: node)
        let restoredNode = snapshot.toInternalPaneNode()
        #expect(restoredNode.allPanes().count == 1)
        #expect(restoredNode.allPanes().first?.title == "test-pane")
    }

    @Test("split snapshot round-trip")
    func splitSnapshot() {
        let p1 = TerminalPaneState(projectPath: testPath)
        let p2 = TerminalPaneState(projectPath: testPath)
        let node = InternalPaneNode.split(InternalBranch(direction: .horizontal, first: .pane(p1), second: .pane(p2)))
        let snapshot = InternalPaneNodeSnapshot(node: node)
        let restoredNode = snapshot.toInternalPaneNode()
        let panes = restoredNode.allPanes()
        #expect(panes.count == 2)
    }

    @Test("nested split snapshot round-trip")
    func nestedSplitSnapshot() {
        let p1 = TerminalPaneState(projectPath: testPath)
        let p2 = TerminalPaneState(projectPath: testPath)
        let p3 = TerminalPaneState(projectPath: testPath)
        let inner = InternalBranch(direction: .vertical, first: .pane(p2), second: .pane(p3))
        let node = InternalPaneNode.split(InternalBranch(direction: .horizontal, first: .pane(p1), second: .split(inner)))
        let snapshot = InternalPaneNodeSnapshot(node: node)
        let restoredNode = snapshot.toInternalPaneNode()
        #expect(restoredNode.allPanes().count == 3)
    }

    @Test("TerminalTabSnapshot with internal panes round-trips")
    func tabSnapshotWithInternalPanes() {
        let pane = TerminalPaneState(projectPath: testPath)
        let tab = TerminalTab(pane: pane)
        let p2 = TerminalPaneState(projectPath: testPath)
        tab.internalPanes = .split(InternalBranch(direction: .horizontal, first: .pane(pane), second: .pane(p2)))
        tab.focusedPaneID = p2.id

        let tabSnapshot = tab.snapshot()
        let restoredTab = TerminalTab(restoring: tabSnapshot)

        #expect(restoredTab.internalPanes != nil)
        #expect(restoredTab.internalPanes!.allPanes().count == 2)
        #expect(restoredTab.focusedPaneID == p2.id)
    }

    @Test("TerminalTabSnapshot without internal panes decodes as nil")
    func tabSnapshotWithoutInternalPanes() {
        let pane = TerminalPaneState(projectPath: testPath)
        let tab = TerminalTab(pane: pane)
        let tabSnapshot = tab.snapshot()
        let restoredTab = TerminalTab(restoring: tabSnapshot)
        #expect(restoredTab.internalPanes == nil)
        #expect(restoredTab.focusedPaneID == nil)
    }
}
