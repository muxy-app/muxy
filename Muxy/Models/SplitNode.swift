import CoreGraphics
import Foundation

enum SplitDirection {
    case horizontal
    case vertical
}

enum SplitPosition {
    case first
    case second
}

enum SplitNode: Identifiable {
    case tabArea(TabArea)
    indirect case split(SplitBranch)

    var id: UUID {
        switch self {
        case let .tabArea(area): area.id
        case let .split(branch): branch.id
        }
    }
}

@Observable
final class SplitBranch: Identifiable {
    let id = UUID()
    var direction: SplitDirection
    var ratio: CGFloat
    var first: SplitNode
    var second: SplitNode

    init(
        direction: SplitDirection,
        ratio: CGFloat = 0.5,
        first: SplitNode,
        second: SplitNode
    ) {
        self.direction = direction
        self.ratio = ratio
        self.first = first
        self.second = second
    }
}

@MainActor
extension SplitNode {
    func splitting(
        areaID: UUID,
        direction: SplitDirection,
        position: SplitPosition,
        projectPath: String
    ) -> (node: SplitNode, newAreaID: UUID?) {
        switch self {
        case let .tabArea(area) where area.id == areaID:
            let newArea = TabArea(projectPath: projectPath)
            let first: SplitNode = position == .first ? .tabArea(newArea) : .tabArea(area)
            let second: SplitNode = position == .first ? .tabArea(area) : .tabArea(newArea)
            let node = SplitNode.split(SplitBranch(
                direction: direction,
                first: first,
                second: second
            ))
            return (node, newArea.id)
        case .tabArea:
            return (self, nil)
        case let .split(branch):
            let (newFirst, id1) = branch.first.splitting(
                areaID: areaID,
                direction: direction,
                position: position,
                projectPath: projectPath
            )
            branch.first = newFirst
            if id1 != nil { return (.split(branch), id1) }
            let (newSecond, id2) = branch.second.splitting(
                areaID: areaID,
                direction: direction,
                position: position,
                projectPath: projectPath
            )
            branch.second = newSecond
            return (.split(branch), id2)
        }
    }

    func splittingWithTab(
        areaID: UUID,
        direction: SplitDirection,
        position: SplitPosition,
        tab: TerminalTab,
        projectPath: String
    ) -> (node: SplitNode, newAreaID: UUID?) {
        switch self {
        case let .tabArea(area) where area.id == areaID:
            let newArea = TabArea(projectPath: projectPath, existingTab: tab)
            let first: SplitNode = position == .first ? .tabArea(newArea) : .tabArea(area)
            let second: SplitNode = position == .first ? .tabArea(area) : .tabArea(newArea)
            let node = SplitNode.split(SplitBranch(direction: direction, first: first, second: second))
            return (node, newArea.id)
        case .tabArea:
            return (self, nil)
        case let .split(branch):
            let (newFirst, id1) = branch.first.splittingWithTab(
                areaID: areaID, direction: direction, position: position, tab: tab, projectPath: projectPath
            )
            branch.first = newFirst
            if id1 != nil { return (.split(branch), id1) }
            let (newSecond, id2) = branch.second.splittingWithTab(
                areaID: areaID, direction: direction, position: position, tab: tab, projectPath: projectPath
            )
            branch.second = newSecond
            return (.split(branch), id2)
        }
    }

    func removing(areaID: UUID) -> SplitNode? {
        switch self {
        case let .tabArea(area) where area.id == areaID:
            return nil
        case .tabArea:
            return self
        case let .split(branch):
            if case let .tabArea(a) = branch.first, a.id == areaID {
                return branch.second
            }
            if case let .tabArea(a) = branch.second, a.id == areaID {
                return branch.first
            }
            if branch.first.containsArea(id: areaID),
               let newFirst = branch.first.removing(areaID: areaID)
            {
                branch.first = newFirst
                return .split(branch)
            }
            if branch.second.containsArea(id: areaID),
               let newSecond = branch.second.removing(areaID: areaID)
            {
                branch.second = newSecond
                return .split(branch)
            }
            return self
        }
    }

    func containsArea(id: UUID) -> Bool {
        switch self {
        case let .tabArea(area): area.id == id
        case let .split(branch):
            branch.first.containsArea(id: id) || branch.second.containsArea(id: id)
        }
    }

    func allAreas() -> [TabArea] {
        switch self {
        case let .tabArea(area): [area]
        case let .split(branch):
            branch.first.allAreas() + branch.second.allAreas()
        }
    }

    func findArea(id: UUID) -> TabArea? {
        switch self {
        case let .tabArea(area): area.id == id ? area : nil
        case let .split(branch):
            branch.first.findArea(id: id) ?? branch.second.findArea(id: id)
        }
    }

    func areaFrames(in rect: CGRect = CGRect(x: 0, y: 0, width: 1, height: 1)) -> [UUID: CGRect] {
        switch self {
        case let .tabArea(area):
            return [area.id: rect]
        case let .split(branch):
            let ratio = min(max(branch.ratio, 0), 1)
            if branch.direction == .horizontal {
                let firstWidth = rect.width * ratio
                let firstRect = CGRect(x: rect.minX, y: rect.minY, width: firstWidth, height: rect.height)
                let secondRect = CGRect(x: rect.minX + firstWidth, y: rect.minY, width: rect.width - firstWidth, height: rect.height)
                return branch.first.areaFrames(in: firstRect).merging(branch.second.areaFrames(in: secondRect)) { current, _ in current }
            }

            let firstHeight = rect.height * ratio
            let firstRect = CGRect(x: rect.minX, y: rect.minY, width: rect.width, height: firstHeight)
            let secondRect = CGRect(x: rect.minX, y: rect.minY + firstHeight, width: rect.width, height: rect.height - firstHeight)
            return branch.first.areaFrames(in: firstRect).merging(branch.second.areaFrames(in: secondRect)) { current, _ in current }
        }
    }
}
