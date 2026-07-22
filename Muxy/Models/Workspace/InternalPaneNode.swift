import CoreGraphics
import Foundation

enum InternalPaneNode: Identifiable {
    case pane(TerminalPaneState)
    indirect case split(InternalBranch)

    var id: UUID {
        switch self {
        case let .pane(pane): pane.id
        case let .split(branch): branch.id
        }
    }
}

@Observable
final class InternalBranch: Identifiable {
    let id = UUID()
    var direction: SplitDirection
    var ratio: CGFloat
    var first: InternalPaneNode
    var second: InternalPaneNode

    init(
        direction: SplitDirection,
        ratio: CGFloat = 0.5,
        first: InternalPaneNode,
        second: InternalPaneNode
    ) {
        self.direction = direction
        self.ratio = ratio
        self.first = first
        self.second = second
    }
}

@MainActor
extension InternalPaneNode {
    func splitting(
        paneID: UUID,
        direction: SplitDirection,
        position: SplitPosition
    ) -> (node: InternalPaneNode, newPaneID: UUID?) {
        switch self {
        case let .pane(pane) where pane.id == paneID:
            let newPane = TerminalPaneState(projectPath: pane.projectPath)
            let first: InternalPaneNode = position == .first ? .pane(newPane) : .pane(pane)
            let second: InternalPaneNode = position == .first ? .pane(pane) : .pane(newPane)
            let node = InternalPaneNode.split(InternalBranch(
                direction: direction,
                first: first,
                second: second
            ))
            return (node, newPane.id)
        case .pane:
            return (self, nil)
        case let .split(branch):
            let (newFirst, id1) = branch.first.splitting(
                paneID: paneID, direction: direction, position: position
            )
            branch.first = newFirst
            if id1 != nil { return (.split(branch), id1) }
            let (newSecond, id2) = branch.second.splitting(
                paneID: paneID, direction: direction, position: position
            )
            branch.second = newSecond
            return (.split(branch), id2)
        }
    }

    func removing(paneID: UUID) -> InternalPaneNode? {
        switch self {
        case let .pane(pane) where pane.id == paneID:
            return nil
        case .pane:
            return self
        case let .split(branch):
            if case let .pane(p) = branch.first, p.id == paneID {
                return branch.second
            }
            if case let .pane(p) = branch.second, p.id == paneID {
                return branch.first
            }
            if branch.first.containsPane(id: paneID),
               let newFirst = branch.first.removing(paneID: paneID)
            {
                branch.first = newFirst
                return .split(branch)
            }
            if branch.second.containsPane(id: paneID),
               let newSecond = branch.second.removing(paneID: paneID)
            {
                branch.second = newSecond
                return .split(branch)
            }
            return self
        }
    }

    func containsPane(id: UUID) -> Bool {
        switch self {
        case let .pane(pane): pane.id == id
        case let .split(branch):
            branch.first.containsPane(id: id) || branch.second.containsPane(id: id)
        }
    }

    func allPanes() -> [TerminalPaneState] {
        switch self {
        case let .pane(pane): [pane]
        case let .split(branch):
            branch.first.allPanes() + branch.second.allPanes()
        }
    }

    func findPane(id: UUID) -> TerminalPaneState? {
        switch self {
        case let .pane(pane): pane.id == id ? pane : nil
        case let .split(branch):
            branch.first.findPane(id: id) ?? branch.second.findPane(id: id)
        }
    }

    func areaFrames(in rect: CGRect = CGRect(x: 0, y: 0, width: 1, height: 1)) -> [UUID: CGRect] {
        switch self {
        case let .pane(pane):
            return [pane.id: rect]
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
