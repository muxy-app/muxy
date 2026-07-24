import CoreGraphics
import Foundation

enum TopLevelTabMoveRequest {
    case toGroup(tabID: UUID, sourceGroupID: UUID, destinationGroupID: UUID)
    case toNewSplit(tabID: UUID, sourceGroupID: UUID, targetGroupID: UUID, split: SplitPlacement)
}

enum TopLevelTabNode: Identifiable {
    case group(TopLevelTabGroup)
    indirect case split(TopLevelTabBranch)

    var id: UUID {
        switch self {
        case let .group(group):
            group.id
        case let .split(branch):
            branch.id
        }
    }
}

@Observable
final class TopLevelTabGroup: Identifiable {
    let id: UUID
    var tabIDs: [UUID]
    var activeTabID: UUID?

    init(id: UUID = UUID(), tabIDs: [UUID], activeTabID: UUID?) {
        self.id = id
        self.tabIDs = tabIDs
        self.activeTabID = activeTabID
    }
}

@Observable
final class TopLevelTabBranch: Identifiable {
    let id: UUID
    var direction: SplitDirection
    var ratio: CGFloat
    var first: TopLevelTabNode
    var second: TopLevelTabNode

    init(
        id: UUID = UUID(),
        direction: SplitDirection,
        ratio: CGFloat = 0.5,
        first: TopLevelTabNode,
        second: TopLevelTabNode
    ) {
        self.id = id
        self.direction = direction
        self.ratio = ratio
        self.first = first
        self.second = second
    }
}

@MainActor
extension TopLevelTabNode {
    var isSingleGroup: Bool {
        if case .group = self {
            return true
        }
        return false
    }

    func allGroups() -> [TopLevelTabGroup] {
        switch self {
        case let .group(group):
            [group]
        case let .split(branch):
            branch.first.allGroups() + branch.second.allGroups()
        }
    }

    func group(id: UUID) -> TopLevelTabGroup? {
        switch self {
        case let .group(group):
            group.id == id ? group : nil
        case let .split(branch):
            branch.first.group(id: id) ?? branch.second.group(id: id)
        }
    }

    func group(containingTabID tabID: UUID) -> TopLevelTabGroup? {
        allGroups().first { $0.tabIDs.contains(tabID) }
    }

    func flattenedTabIDs() -> [UUID] {
        allGroups().flatMap(\.tabIDs)
    }

    func groupFrames(
        in rect: CGRect = CGRect(x: 0, y: 0, width: 1, height: 1)
    ) -> [UUID: CGRect] {
        switch self {
        case let .group(group):
            return [group.id: rect]
        case let .split(branch):
            let ratio = min(max(branch.ratio, 0), 1)
            if branch.direction == .horizontal {
                let firstWidth = rect.width * ratio
                let firstRect = CGRect(
                    x: rect.minX,
                    y: rect.minY,
                    width: firstWidth,
                    height: rect.height
                )
                let secondRect = CGRect(
                    x: rect.minX + firstWidth,
                    y: rect.minY,
                    width: rect.width - firstWidth,
                    height: rect.height
                )
                return branch.first.groupFrames(in: firstRect)
                    .merging(branch.second.groupFrames(in: secondRect)) { current, _ in current }
            }
            let firstHeight = rect.height * ratio
            let firstRect = CGRect(
                x: rect.minX,
                y: rect.minY,
                width: rect.width,
                height: firstHeight
            )
            let secondRect = CGRect(
                x: rect.minX,
                y: rect.minY + firstHeight,
                width: rect.width,
                height: rect.height - firstHeight
            )
            return branch.first.groupFrames(in: firstRect)
                .merging(branch.second.groupFrames(in: secondRect)) { current, _ in current }
        }
    }

    func insertingSplit(
        aroundGroupID groupID: UUID,
        newGroup: TopLevelTabGroup,
        placement: SplitPlacement
    ) -> TopLevelTabNode {
        switch self {
        case let .group(group) where group.id == groupID:
            let existing = TopLevelTabNode.group(group)
            let inserted = TopLevelTabNode.group(newGroup)
            let first = placement.position == .first ? inserted : existing
            let second = placement.position == .first ? existing : inserted
            return .split(TopLevelTabBranch(
                direction: placement.direction,
                first: first,
                second: second
            ))
        case .group:
            return self
        case let .split(branch):
            if branch.first.group(id: groupID) != nil {
                branch.first = branch.first.insertingSplit(
                    aroundGroupID: groupID,
                    newGroup: newGroup,
                    placement: placement
                )
                return .split(branch)
            }
            branch.second = branch.second.insertingSplit(
                aroundGroupID: groupID,
                newGroup: newGroup,
                placement: placement
            )
            return .split(branch)
        }
    }

    func removingGroup(id groupID: UUID) -> TopLevelTabNode? {
        switch self {
        case let .group(group):
            return group.id == groupID ? nil : self
        case let .split(branch):
            let first = branch.first.removingGroup(id: groupID)
            let second = branch.second.removingGroup(id: groupID)
            switch (first, second) {
            case let (first?, second?):
                branch.first = first
                branch.second = second
                return .split(branch)
            case let (first?, nil):
                return first
            case let (nil, second?):
                return second
            case (nil, nil):
                return nil
            }
        }
    }

    func pruningTabs(validTabIDs: Set<UUID>) -> TopLevelTabNode? {
        var seen = Set<UUID>()
        return pruningTabs(validTabIDs: validTabIDs, seen: &seen)
    }

    private func pruningTabs(
        validTabIDs: Set<UUID>,
        seen: inout Set<UUID>
    ) -> TopLevelTabNode? {
        switch self {
        case let .group(group):
            group.tabIDs = group.tabIDs.filter {
                validTabIDs.contains($0) && seen.insert($0).inserted
            }
            guard !group.tabIDs.isEmpty else { return nil }
            if group.activeTabID.map({ group.tabIDs.contains($0) }) != true {
                group.activeTabID = group.tabIDs.first
            }
            return .group(group)
        case let .split(branch):
            let first = branch.first.pruningTabs(validTabIDs: validTabIDs, seen: &seen)
            let second = branch.second.pruningTabs(validTabIDs: validTabIDs, seen: &seen)
            switch (first, second) {
            case let (first?, second?):
                branch.first = first
                branch.second = second
                return .split(branch)
            case let (first?, nil):
                return first
            case let (nil, second?):
                return second
            case (nil, nil):
                return nil
            }
        }
    }
}
