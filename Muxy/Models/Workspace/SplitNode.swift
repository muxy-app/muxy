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

indirect enum VisiblePaneNode: Identifiable {
    case pane(area: TabArea, tab: TerminalTab)
    case split(branch: SplitBranch, first: VisiblePaneNode, second: VisiblePaneNode)

    var id: UUID {
        switch self {
        case let .pane(_, tab): tab.id
        case let .split(branch, _, _): branch.id
        }
    }

    func allPanes() -> [(area: TabArea, tab: TerminalTab)] {
        switch self {
        case let .pane(area, tab):
            [(area, tab)]
        case let .split(_, first, second):
            first.allPanes() + second.allPanes()
        }
    }

    func areaFrames(in rect: CGRect = CGRect(x: 0, y: 0, width: 1, height: 1)) -> [UUID: CGRect] {
        switch self {
        case let .pane(area, _):
            return [area.id: rect]
        case let .split(branch, first, second):
            let ratio = min(max(branch.ratio, 0), 1)
            if branch.direction == .horizontal {
                let firstWidth = rect.width * ratio
                let firstRect = CGRect(x: rect.minX, y: rect.minY, width: firstWidth, height: rect.height)
                let secondRect = CGRect(
                    x: rect.minX + firstWidth,
                    y: rect.minY,
                    width: rect.width - firstWidth,
                    height: rect.height
                )
                return first.areaFrames(in: firstRect)
                    .merging(second.areaFrames(in: secondRect)) { current, _ in current }
            }
            let firstHeight = rect.height * ratio
            let firstRect = CGRect(x: rect.minX, y: rect.minY, width: rect.width, height: firstHeight)
            let secondRect = CGRect(
                x: rect.minX,
                y: rect.minY + firstHeight,
                width: rect.width,
                height: rect.height - firstHeight
            )
            return first.areaFrames(in: firstRect)
                .merging(second.areaFrames(in: secondRect)) { current, _ in current }
        }
    }
}

struct FlatTabLocation {
    let area: TabArea
    let tab: TerminalTab
    let slotArea: TabArea
    let slotIndex: Int
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
    func allTabs() -> [TerminalTab] {
        allAreas().flatMap(\.tabs)
    }

    func locateTab(id: UUID) -> (area: TabArea, tab: TerminalTab)? {
        for area in allAreas() {
            guard let tab = area.tabs.first(where: { $0.id == id }) else { continue }
            return (area, tab)
        }
        return nil
    }

    func visibleLayout(forTopLevelTabID topLevelTabID: UUID) -> VisiblePaneNode? {
        switch self {
        case let .tabArea(area):
            let matchingTabs = area.tabs.filter {
                $0.id == topLevelTabID || $0.parentTabID == topLevelTabID
            }
            guard !matchingTabs.isEmpty else { return nil }
            let tab = matchingTabs.first(where: { $0.id == area.activeTabID })
                ?? matchingTabs.first(where: { $0.id == topLevelTabID })
                ?? matchingTabs[0]
            return .pane(area: area, tab: tab)
        case let .split(branch):
            let first = branch.first.visibleLayout(forTopLevelTabID: topLevelTabID)
            let second = branch.second.visibleLayout(forTopLevelTabID: topLevelTabID)
            switch (first, second) {
            case let (first?, second?):
                return .split(branch: branch, first: first, second: second)
            case let (first?, nil):
                return first
            case let (nil, second?):
                return second
            case (nil, nil):
                return nil
            }
        }
    }

    func topLevelTabs(order: [UUID] = []) -> [(area: TabArea, tab: TerminalTab)] {
        let located = allAreas().flatMap { area in
            area.tabs.compactMap { tab in
                tab.parentTabID == nil ? (area: area, tab: tab) : nil
            }
        }
        guard !order.isEmpty else { return located }
        let positions = Dictionary(uniqueKeysWithValues: order.enumerated().map { ($1, $0) })
        return located.enumerated().sorted {
            let left = positions[$0.element.tab.id] ?? order.count + $0.offset
            let right = positions[$1.element.tab.id] ?? order.count + $1.offset
            return left < right
        }.map(\.element)
    }

    func flatTabLocations(topLevelOrder: [UUID]) -> [FlatTabLocation] {
        let traversed = allAreas().flatMap { area in
            area.tabs.enumerated().map { index, tab in
                FlatTabLocation(
                    area: area,
                    tab: tab,
                    slotArea: area,
                    slotIndex: index
                )
            }
        }
        var orderedRoots = topLevelTabs(order: topLevelOrder).makeIterator()
        return traversed.map { location in
            guard location.tab.parentTabID == nil else { return location }
            guard let root = orderedRoots.next() else { return location }
            return FlatTabLocation(
                area: root.area,
                tab: root.tab,
                slotArea: location.slotArea,
                slotIndex: location.slotIndex
            )
        }
    }

    func splitting(
        areaID: UUID,
        direction: SplitDirection,
        position: SplitPosition,
        command: String? = nil
    ) -> (node: SplitNode, newAreaID: UUID?) {
        switch self {
        case let .tabArea(area) where area.id == areaID:
            guard let activeTab = area.activeTab else { return (self, nil) }
            let topLevelTabID = activeTab.parentTabID ?? activeTab.id
            let newArea = TabArea(
                projectPath: area.projectPath,
                command: command,
                parentTabID: topLevelTabID
            )
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
                command: command
            )
            branch.first = newFirst
            if id1 != nil {
                return (.split(branch), id1)
            }
            let (newSecond, id2) = branch.second.splitting(
                areaID: areaID,
                direction: direction,
                position: position,
                command: command
            )
            branch.second = newSecond
            return (.split(branch), id2)
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
            let node = SplitNode.split(SplitBranch(direction: direction, first: first, second: second))
            return (node, newArea.id)
        case .tabArea:
            return (self, nil)
        case let .split(branch):
            let (newFirst, id1) = branch.first.splittingWithTab(
                areaID: areaID, direction: direction, position: position, tab: tab
            )
            branch.first = newFirst
            if id1 != nil {
                return (.split(branch), id1)
            }
            let (newSecond, id2) = branch.second.splittingWithTab(
                areaID: areaID, direction: direction, position: position, tab: tab
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
