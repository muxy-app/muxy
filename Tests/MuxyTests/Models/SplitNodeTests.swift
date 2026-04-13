import CoreGraphics
import Foundation
import Testing

@testable import Muxy

@Suite("SplitNode")
@MainActor
struct SplitNodeTests {
    private let testPath = "/tmp/test"

    @Test("tabArea node id matches area id")
    func tabAreaNodeID() {
        let area = TabArea(projectPath: testPath)
        let node = SplitNode.tabArea(area)
        #expect(node.id == area.id)
    }

    @Test("split node id matches branch id")
    func splitNodeID() {
        let branch = SplitBranch(
            direction: .horizontal,
            first: .tabArea(TabArea(projectPath: testPath)),
            second: .tabArea(TabArea(projectPath: testPath))
        )
        let node = SplitNode.split(branch)
        #expect(node.id == branch.id)
    }

    @Test("allAreas returns single area for leaf node")
    func allAreasSingle() {
        let area = TabArea(projectPath: testPath)
        let node = SplitNode.tabArea(area)
        let areas = node.allAreas()
        #expect(areas.count == 1)
        #expect(areas[0].id == area.id)
    }

    @Test("allAreas returns all leaves in split tree")
    func allAreasSplit() {
        let a1 = TabArea(projectPath: testPath)
        let a2 = TabArea(projectPath: testPath)
        let a3 = TabArea(projectPath: testPath)
        let inner = SplitBranch(direction: .vertical, first: .tabArea(a2), second: .tabArea(a3))
        let root = SplitNode.split(SplitBranch(
            direction: .horizontal,
            first: .tabArea(a1),
            second: .split(inner)
        ))
        let areas = root.allAreas()
        #expect(areas.count == 3)
        #expect(areas.map(\.id).contains(a1.id))
        #expect(areas.map(\.id).contains(a2.id))
        #expect(areas.map(\.id).contains(a3.id))
    }

    @Test("containsArea returns true when area exists")
    func containsAreaTrue() {
        let a1 = TabArea(projectPath: testPath)
        let a2 = TabArea(projectPath: testPath)
        let node = SplitNode.split(SplitBranch(
            direction: .horizontal,
            first: .tabArea(a1),
            second: .tabArea(a2)
        ))
        #expect(node.containsArea(id: a2.id))
    }

    @Test("containsArea returns false when area missing")
    func containsAreaFalse() {
        let area = TabArea(projectPath: testPath)
        let node = SplitNode.tabArea(area)
        #expect(!node.containsArea(id: UUID()))
    }

    @Test("findArea returns area when found")
    func findAreaFound() {
        let a1 = TabArea(projectPath: testPath)
        let a2 = TabArea(projectPath: testPath)
        let node = SplitNode.split(SplitBranch(
            direction: .horizontal,
            first: .tabArea(a1),
            second: .tabArea(a2)
        ))
        let found = node.findArea(id: a2.id)
        #expect(found?.id == a2.id)
    }

    @Test("findArea returns nil when not found")
    func findAreaNotFound() {
        let area = TabArea(projectPath: testPath)
        let node = SplitNode.tabArea(area)
        #expect(node.findArea(id: UUID()) == nil)
    }

    @Test("splitting creates new area in second position")
    func splittingSecondPosition() {
        let area = TabArea(projectPath: testPath)
        let node = SplitNode.tabArea(area)
        let (result, newAreaID) = node.splitting(
            areaID: area.id,
            direction: .horizontal,
            position: .second
        )
        #expect(newAreaID != nil)
        #expect(newAreaID != area.id)
        if case let .split(branch) = result {
            #expect(branch.direction == .horizontal)
            if case let .tabArea(first) = branch.first {
                #expect(first.id == area.id)
            } else {
                Issue.record("First child should be original area")
            }
            if case let .tabArea(second) = branch.second {
                #expect(second.id == newAreaID)
            } else {
                Issue.record("Second child should be new area")
            }
        } else {
            Issue.record("Result should be a split")
        }
    }

    @Test("splitting creates new area in first position")
    func splittingFirstPosition() {
        let area = TabArea(projectPath: testPath)
        let node = SplitNode.tabArea(area)
        let (result, newAreaID) = node.splitting(
            areaID: area.id,
            direction: .vertical,
            position: .first
        )
        #expect(newAreaID != nil)
        if case let .split(branch) = result {
            if case let .tabArea(first) = branch.first {
                #expect(first.id == newAreaID)
            } else {
                Issue.record("First child should be new area")
            }
        } else {
            Issue.record("Result should be a split")
        }
    }

    @Test("splitting on nonexistent area returns self unchanged")
    func splittingNonexistent() {
        let area = TabArea(projectPath: testPath)
        let node = SplitNode.tabArea(area)
        let (result, newAreaID) = node.splitting(
            areaID: UUID(),
            direction: .horizontal,
            position: .second
        )
        #expect(newAreaID == nil)
        #expect(result.id == area.id)
    }

    @Test("splitting in nested tree finds target area")
    func splittingNested() {
        let a1 = TabArea(projectPath: testPath)
        let a2 = TabArea(projectPath: testPath)
        let root = SplitNode.split(SplitBranch(
            direction: .horizontal,
            first: .tabArea(a1),
            second: .tabArea(a2)
        ))
        let (_, newAreaID) = root.splitting(
            areaID: a2.id,
            direction: .vertical,
            position: .second
        )
        #expect(newAreaID != nil)
        #expect(root.allAreas().count == 3)
    }

    @Test("splittingWithTab creates area with the provided tab")
    func splittingWithTab() {
        let area = TabArea(projectPath: testPath)
        let tab = TerminalTab(pane: TerminalPaneState(projectPath: testPath))
        let node = SplitNode.tabArea(area)
        let (result, newAreaID) = node.splittingWithTab(
            areaID: area.id,
            direction: .horizontal,
            position: .second,
            tab: tab
        )
        #expect(newAreaID != nil)
        if case let .split(branch) = result {
            if case let .tabArea(newArea) = branch.second {
                #expect(newArea.tabs.count == 1)
                #expect(newArea.tabs[0].id == tab.id)
            } else {
                Issue.record("Second child should be new area with tab")
            }
        } else {
            Issue.record("Result should be a split")
        }
    }

    @Test("removing sole leaf returns nil")
    func removingSoleLeaf() {
        let area = TabArea(projectPath: testPath)
        let node = SplitNode.tabArea(area)
        let result = node.removing(areaID: area.id)
        #expect(result == nil)
    }

    @Test("removing left child from split returns right child")
    func removingLeftChild() {
        let a1 = TabArea(projectPath: testPath)
        let a2 = TabArea(projectPath: testPath)
        let node = SplitNode.split(SplitBranch(
            direction: .horizontal,
            first: .tabArea(a1),
            second: .tabArea(a2)
        ))
        let result = node.removing(areaID: a1.id)
        #expect(result != nil)
        if case let .tabArea(remaining) = result {
            #expect(remaining.id == a2.id)
        } else {
            Issue.record("Should collapse to remaining area")
        }
    }

    @Test("removing right child from split returns left child")
    func removingRightChild() {
        let a1 = TabArea(projectPath: testPath)
        let a2 = TabArea(projectPath: testPath)
        let node = SplitNode.split(SplitBranch(
            direction: .horizontal,
            first: .tabArea(a1),
            second: .tabArea(a2)
        ))
        let result = node.removing(areaID: a2.id)
        #expect(result != nil)
        if case let .tabArea(remaining) = result {
            #expect(remaining.id == a1.id)
        } else {
            Issue.record("Should collapse to remaining area")
        }
    }

    @Test("removing nested area restructures tree")
    func removingNested() {
        let a1 = TabArea(projectPath: testPath)
        let a2 = TabArea(projectPath: testPath)
        let a3 = TabArea(projectPath: testPath)
        let inner = SplitBranch(direction: .vertical, first: .tabArea(a2), second: .tabArea(a3))
        let root = SplitNode.split(SplitBranch(
            direction: .horizontal,
            first: .tabArea(a1),
            second: .split(inner)
        ))
        let result = root.removing(areaID: a2.id)
        #expect(result != nil)
        let areas = result!.allAreas()
        #expect(areas.count == 2)
        #expect(areas.map(\.id).contains(a1.id))
        #expect(areas.map(\.id).contains(a3.id))
    }

    @Test("removing nonexistent area returns self")
    func removingNonexistent() {
        let area = TabArea(projectPath: testPath)
        let node = SplitNode.tabArea(area)
        let result = node.removing(areaID: UUID())
        #expect(result?.id == area.id)
    }

    @Test("areaFrames single area returns unit rect")
    func areaFramesSingle() {
        let area = TabArea(projectPath: testPath)
        let node = SplitNode.tabArea(area)
        let frames = node.areaFrames()
        #expect(frames.count == 1)
        #expect(frames[area.id] == CGRect(x: 0, y: 0, width: 1, height: 1))
    }

    @Test("areaFrames horizontal split divides width")
    func areaFramesHorizontal() {
        let a1 = TabArea(projectPath: testPath)
        let a2 = TabArea(projectPath: testPath)
        let node = SplitNode.split(SplitBranch(
            direction: .horizontal,
            ratio: 0.5,
            first: .tabArea(a1),
            second: .tabArea(a2)
        ))
        let frames = node.areaFrames()
        #expect(frames[a1.id]?.width == 0.5)
        #expect(frames[a2.id]?.width == 0.5)
        #expect(frames[a1.id]?.minX == 0)
        #expect(frames[a2.id]?.minX == 0.5)
        #expect(frames[a1.id]?.height == 1)
        #expect(frames[a2.id]?.height == 1)
    }

    @Test("areaFrames vertical split divides height")
    func areaFramesVertical() {
        let a1 = TabArea(projectPath: testPath)
        let a2 = TabArea(projectPath: testPath)
        let node = SplitNode.split(SplitBranch(
            direction: .vertical,
            ratio: 0.5,
            first: .tabArea(a1),
            second: .tabArea(a2)
        ))
        let frames = node.areaFrames()
        #expect(frames[a1.id]?.height == 0.5)
        #expect(frames[a2.id]?.height == 0.5)
        #expect(frames[a1.id]?.minY == 0)
        #expect(frames[a2.id]?.minY == 0.5)
        #expect(frames[a1.id]?.width == 1)
    }

    @Test("areaFrames respects custom ratio")
    func areaFramesCustomRatio() {
        let a1 = TabArea(projectPath: testPath)
        let a2 = TabArea(projectPath: testPath)
        let node = SplitNode.split(SplitBranch(
            direction: .horizontal,
            ratio: 0.3,
            first: .tabArea(a1),
            second: .tabArea(a2)
        ))
        let frames = node.areaFrames()
        let a1Width = frames[a1.id]!.width
        let a2Width = frames[a2.id]!.width
        #expect(abs(a1Width - 0.3) < 0.001)
        #expect(abs(a2Width - 0.7) < 0.001)
    }

    @Test("areaFrames nested splits calculate correctly")
    func areaFramesNested() {
        let a1 = TabArea(projectPath: testPath)
        let a2 = TabArea(projectPath: testPath)
        let a3 = TabArea(projectPath: testPath)
        let inner = SplitBranch(direction: .vertical, ratio: 0.5, first: .tabArea(a2), second: .tabArea(a3))
        let root = SplitNode.split(SplitBranch(
            direction: .horizontal,
            ratio: 0.5,
            first: .tabArea(a1),
            second: .split(inner)
        ))
        let frames = root.areaFrames()
        #expect(frames.count == 3)
        #expect(frames[a1.id]?.width == 0.5)
        #expect(frames[a2.id]?.width == 0.5)
        #expect(frames[a2.id]?.height == 0.5)
        #expect(frames[a3.id]?.height == 0.5)
        #expect(frames[a3.id]?.minY == 0.5)
    }
}
