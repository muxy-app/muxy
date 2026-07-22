import CoreGraphics
import Foundation
import Testing

@testable import Muxy

@Suite("InternalPaneNode")
@MainActor
struct InternalPaneNodeTests {
    private let testPath = "/tmp/test"

    @Test("pane node returns pane id")
    func paneNodeID() {
        let pane = TerminalPaneState(projectPath: testPath)
        let node = InternalPaneNode.pane(pane)
        #expect(node.id == pane.id)
    }

    @Test("split node returns branch id")
    func splitNodeID() {
        let branch = InternalBranch(
            direction: .horizontal,
            first: .pane(TerminalPaneState(projectPath: testPath)),
            second: .pane(TerminalPaneState(projectPath: testPath))
        )
        let node = InternalPaneNode.split(branch)
        #expect(node.id == branch.id)
    }

    @Test("allPanes returns single pane for leaf")
    func allPanesSingle() {
        let pane = TerminalPaneState(projectPath: testPath)
        let node = InternalPaneNode.pane(pane)
        let panes = node.allPanes()
        #expect(panes.count == 1)
        #expect(panes[0].id == pane.id)
    }

    @Test("allPanes returns all panes in split tree")
    func allPanesSplit() {
        let p1 = TerminalPaneState(projectPath: testPath)
        let p2 = TerminalPaneState(projectPath: testPath)
        let p3 = TerminalPaneState(projectPath: testPath)
        let inner = InternalBranch(direction: .vertical, first: .pane(p2), second: .pane(p3))
        let root = InternalPaneNode.split(InternalBranch(
            direction: .horizontal,
            first: .pane(p1),
            second: .split(inner)
        ))
        let panes = root.allPanes()
        #expect(panes.count == 3)
        #expect(panes.map(\.id).contains(p1.id))
        #expect(panes.map(\.id).contains(p2.id))
        #expect(panes.map(\.id).contains(p3.id))
    }

    @Test("containsPane returns true when pane exists")
    func containsPaneTrue() {
        let p1 = TerminalPaneState(projectPath: testPath)
        let p2 = TerminalPaneState(projectPath: testPath)
        let node = InternalPaneNode.split(InternalBranch(
            direction: .horizontal,
            first: .pane(p1),
            second: .pane(p2)
        ))
        #expect(node.containsPane(id: p2.id))
    }

    @Test("containsPane returns false when pane missing")
    func containsPaneFalse() {
        let pane = TerminalPaneState(projectPath: testPath)
        let node = InternalPaneNode.pane(pane)
        #expect(!node.containsPane(id: UUID()))
    }

    @Test("findPane returns pane when found")
    func findPaneFound() {
        let p1 = TerminalPaneState(projectPath: testPath)
        let p2 = TerminalPaneState(projectPath: testPath)
        let node = InternalPaneNode.split(InternalBranch(
            direction: .horizontal,
            first: .pane(p1),
            second: .pane(p2)
        ))
        let found = node.findPane(id: p2.id)
        #expect(found != nil)
        #expect(found?.id == p2.id)
    }

    @Test("findPane returns nil when not found")
    func findPaneNotFound() {
        let pane = TerminalPaneState(projectPath: testPath)
        let node = InternalPaneNode.pane(pane)
        #expect(node.findPane(id: UUID()) == nil)
    }

    @Test("splitting leaf node creates split branch")
    func splittingLeaf() {
        let pane = TerminalPaneState(projectPath: testPath)
        let node = InternalPaneNode.pane(pane)
        let (newNode, newPaneID) = node.splitting(
            paneID: pane.id,
            direction: .horizontal,
            position: .second
        )
        #expect(newPaneID != nil)
        #expect(newPaneID != pane.id)
        let panes = newNode.allPanes()
        #expect(panes.count == 2)
        #expect(panes.map(\.id).contains(pane.id))
        #expect(panes.map(\.id).contains(newPaneID))
    }

    @Test("splitting unknown pane id returns self")
    func splittingUnknown() {
        let pane = TerminalPaneState(projectPath: testPath)
        let node = InternalPaneNode.pane(pane)
        let (newNode, newPaneID) = node.splitting(
            paneID: UUID(),
            direction: .horizontal,
            position: .second
        )
        #expect(newPaneID == nil)
        if case .pane = newNode { /* expected */ } else { #expect(Bool(false), "Expected .pane") }
    }

    @Test("splitting split node at specific pane")
    func splittingSplitNode() {
        let p1 = TerminalPaneState(projectPath: testPath)
        let p2 = TerminalPaneState(projectPath: testPath)
        let node = InternalPaneNode.split(InternalBranch(
            direction: .horizontal, first: .pane(p1), second: .pane(p2)
        ))
        let (newNode, newPaneID) = node.splitting(
            paneID: p2.id,
            direction: .vertical,
            position: .first
        )
        #expect(newPaneID != nil)
        let panes = newNode.allPanes()
        #expect(panes.count == 3)
    }

    @Test("removing pane from split collapses to single pane")
    func removingSingleCollapse() {
        let p1 = TerminalPaneState(projectPath: testPath)
        let p2 = TerminalPaneState(projectPath: testPath)
        let node = InternalPaneNode.split(InternalBranch(
            direction: .horizontal, first: .pane(p1), second: .pane(p2)
        ))
        let newNode = node.removing(paneID: p1.id)
        let panes = newNode?.allPanes()
        #expect(panes?.count == 1)
        #expect(panes?[0].id == p2.id)
    }

    @Test("removing last pane returns nil")
    func removingLastPane() {
        let pane = TerminalPaneState(projectPath: testPath)
        let node = InternalPaneNode.pane(pane)
        let newNode = node.removing(paneID: pane.id)
        #expect(newNode == nil)
    }

    @Test("removing unknown pane returns self")
    func removingUnknown() {
        let pane = TerminalPaneState(projectPath: testPath)
        let node = InternalPaneNode.pane(pane)
        let newNode = node.removing(paneID: UUID())
        #expect(newNode != nil)
    }

    @Test("areaFrames horizontal split")
    func areaFramesHorizontal() {
        let p1 = TerminalPaneState(projectPath: testPath)
        let p2 = TerminalPaneState(projectPath: testPath)
        let branch = InternalBranch(direction: .horizontal, ratio: 0.5, first: .pane(p1), second: .pane(p2))
        let node = InternalPaneNode.split(branch)
        let frames = node.areaFrames(in: CGRect(x: 0, y: 0, width: 200, height: 100))
        #expect(frames.keys.count == 2)
        #expect(frames[p1.id]?.width == 100)
        #expect(frames[p2.id]?.width == 100)
    }

    @Test("areaFrames vertical split")
    func areaFramesVertical() {
        let p1 = TerminalPaneState(projectPath: testPath)
        let p2 = TerminalPaneState(projectPath: testPath)
        let branch = InternalBranch(direction: .vertical, ratio: 0.5, first: .pane(p1), second: .pane(p2))
        let node = InternalPaneNode.split(branch)
        let frames = node.areaFrames(in: CGRect(x: 0, y: 0, width: 200, height: 100))
        #expect(frames.keys.count == 2)
        #expect(frames[p1.id]?.height == 50)
        #expect(frames[p2.id]?.height == 50)
    }
}
