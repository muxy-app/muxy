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
    var children: [SplitNode]
    var ratios: [CGFloat]

    init(
        direction: SplitDirection,
        children: [SplitNode],
        ratios: [CGFloat]? = nil
    ) {
        self.direction = direction
        self.children = children
        self.ratios = ratios ?? Array(repeating: 1.0 / CGFloat(children.count), count: children.count)
    }
}

@MainActor
extension SplitNode {
    func splitting(
        areaID: UUID,
        direction: SplitDirection,
        position: SplitPosition
    ) -> (node: SplitNode, newAreaID: UUID?) {
        switch self {
        case let .tabArea(area) where area.id == areaID:
            let newArea = TabArea(projectPath: area.projectPath)
            let first: SplitNode = position == .first ? .tabArea(newArea) : .tabArea(area)
            let second: SplitNode = position == .first ? .tabArea(area) : .tabArea(newArea)
            let node = SplitNode.split(SplitBranch(
                direction: direction,
                children: [first, second]
            ))
            return (node, newArea.id)
        case .tabArea:
            return (self, nil)
        case let .split(branch):
            for (index, child) in branch.children.enumerated() {
                let (newChild, newID) = child.splitting(
                    areaID: areaID,
                    direction: direction,
                    position: position
                )
                if let newID {
                    if case let .split(newBranch) = newChild, newBranch.direction == branch.direction {
                        branch.children.remove(at: index)
                        for (i, grandchild) in newBranch.children.enumerated() {
                            branch.children.insert(grandchild, at: index + i)
                        }
                    } else {
                        branch.children[index] = newChild
                    }
                    let count = branch.children.count
                    branch.ratios = Array(repeating: 1.0 / CGFloat(count), count: count)
                    return (.split(branch), newID)
                }
            }
            return (self, nil)
        }
    }

    func splittingWithTab(
        areaID: UUID,
        direction: SplitDirection,
        position: SplitPosition,
        tab: TerminalTab
    ) -> (node: SplitNode, newAreaID: UUID?) {
        switch self {
        case let .tabArea(area) where area.id == areaID:
            let newArea = TabArea(projectPath: area.projectPath, existingTab: tab)
            let first: SplitNode = position == .first ? .tabArea(newArea) : .tabArea(area)
            let second: SplitNode = position == .first ? .tabArea(area) : .tabArea(newArea)
            let node = SplitNode.split(SplitBranch(
                direction: direction,
                children: [first, second]
            ))
            return (node, newArea.id)
        case .tabArea:
            return (self, nil)
        case let .split(branch):
            for (index, child) in branch.children.enumerated() {
                let (newChild, newID) = child.splittingWithTab(
                    areaID: areaID,
                    direction: direction,
                    position: position,
                    tab: tab
                )
                if let newID {
                    if case let .split(newBranch) = newChild, newBranch.direction == branch.direction {
                        branch.children.remove(at: index)
                        for (i, grandchild) in newBranch.children.enumerated() {
                            branch.children.insert(grandchild, at: index + i)
                        }
                    } else {
                        branch.children[index] = newChild
                    }
                    let count = branch.children.count
                    branch.ratios = Array(repeating: 1.0 / CGFloat(count), count: count)
                    return (.split(branch), newID)
                }
            }
            return (self, nil)
        }
    }

    func removing(areaID: UUID) -> SplitNode? {
        switch self {
        case let .tabArea(area) where area.id == areaID:
            return nil
        case .tabArea:
            return self
        case let .split(branch):
            var newChildren: [SplitNode] = []
            for child in branch.children {
                if child.containsArea(id: areaID) {
                    if let result = child.removing(areaID: areaID) {
                        if case let .split(childBranch) = result, childBranch.direction == branch.direction {
                            newChildren.append(contentsOf: childBranch.children)
                        } else {
                            newChildren.append(result)
                        }
                    }
                } else {
                    newChildren.append(child)
                }
            }

            if newChildren.isEmpty {
                return nil
            }
            if newChildren.count == 1 {
                return newChildren[0]
            }

            branch.children = newChildren
            let count = newChildren.count
            branch.ratios = Array(repeating: 1.0 / CGFloat(count), count: count)
            return .split(branch)
        }
    }

    func containsArea(id: UUID) -> Bool {
        switch self {
        case let .tabArea(area): area.id == id
        case let .split(branch): branch.children.contains { $0.containsArea(id: id) }
        }
    }

    func allAreas() -> [TabArea] {
        switch self {
        case let .tabArea(area): [area]
        case let .split(branch): branch.children.flatMap { $0.allAreas() }
        }
    }

    func findArea(id: UUID) -> TabArea? {
        switch self {
        case let .tabArea(area): area.id == id ? area : nil
        case let .split(branch): branch.children.compactMap { $0.findArea(id: id) }.first
        }
    }

    func areaFrames(in rect: CGRect = CGRect(x: 0, y: 0, width: 1, height: 1)) -> [UUID: CGRect] {
        switch self {
        case let .tabArea(area):
            return [area.id: rect]
        case let .split(branch):
            let isHorizontal = branch.direction == .horizontal
            var offset: CGFloat = 0
            var frames: [UUID: CGRect] = [:]

            for (index, child) in branch.children.enumerated() {
                let ratio = branch.ratios[index]
                let size = isHorizontal ? rect.width * ratio : rect.height * ratio
                let childRect = if isHorizontal {
                    CGRect(x: rect.minX + offset, y: rect.minY, width: size, height: rect.height)
                } else {
                    CGRect(x: rect.minX, y: rect.minY + offset, width: rect.width, height: size)
                }
                frames.merge(child.areaFrames(in: childRect)) { current, _ in current }
                offset += size
            }

            return frames
        }
    }
}
