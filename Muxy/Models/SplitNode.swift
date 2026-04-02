import Foundation

enum SplitDirection {
    case horizontal
    case vertical
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
    func splitting(areaID: UUID, direction: SplitDirection, projectPath: String) -> (node: SplitNode, newAreaID: UUID?) {
        switch self {
        case let .tabArea(area) where area.id == areaID:
            let newArea = TabArea(projectPath: projectPath)
            let node = SplitNode.split(SplitBranch(
                direction: direction,
                first: .tabArea(area),
                second: .tabArea(newArea)
            ))
            return (node, newArea.id)
        case .tabArea:
            return (self, nil)
        case let .split(branch):
            let (newFirst, id1) = branch.first.splitting(areaID: areaID, direction: direction, projectPath: projectPath)
            let (newSecond, id2) = branch.second.splitting(areaID: areaID, direction: direction, projectPath: projectPath)
            branch.first = newFirst
            branch.second = newSecond
            return (.split(branch), id1 ?? id2)
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
               let newFirst = branch.first.removing(areaID: areaID) {
                branch.first = newFirst
                return .split(branch)
            }
            if branch.second.containsArea(id: areaID),
               let newSecond = branch.second.removing(areaID: areaID) {
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
}
