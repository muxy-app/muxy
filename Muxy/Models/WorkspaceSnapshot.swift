import Foundation

struct WorkspaceSnapshot: Codable {
    let projectID: UUID
    let focusedAreaID: UUID?
    let root: SplitNodeSnapshot
}

indirect enum SplitNodeSnapshot: Codable {
    case tabArea(TabAreaSnapshot)
    case split(SplitBranchSnapshot)

    private enum CodingKeys: String, CodingKey {
        case type
        case tabArea
        case split
    }

    private enum NodeType: String, Codable {
        case tabArea
        case split
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(NodeType.self, forKey: .type)
        switch type {
        case .tabArea:
            self = try .tabArea(container.decode(TabAreaSnapshot.self, forKey: .tabArea))
        case .split:
            self = try .split(container.decode(SplitBranchSnapshot.self, forKey: .split))
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .tabArea(area):
            try container.encode(NodeType.tabArea, forKey: .type)
            try container.encode(area, forKey: .tabArea)
        case let .split(branch):
            try container.encode(NodeType.split, forKey: .type)
            try container.encode(branch, forKey: .split)
        }
    }
}

struct SplitBranchSnapshot: Codable {
    let direction: SplitDirectionSnapshot
    let ratio: Double
    let first: SplitNodeSnapshot
    let second: SplitNodeSnapshot
}

enum SplitDirectionSnapshot: String, Codable {
    case horizontal
    case vertical
}

struct TabAreaSnapshot: Codable {
    let id: UUID
    let projectPath: String
    let tabs: [TerminalTabSnapshot]
    let activeTabIndex: Int?
}

struct TerminalTabSnapshot: Codable {
    let customTitle: String?
    let isPinned: Bool
    let projectPath: String
    let paneTitle: String
}

struct RestoredWorkspace {
    let projectID: UUID
    let root: SplitNode
    let focusedAreaID: UUID
}

@MainActor
enum WorkspaceRestorer {
    static func restoreAll(
        from snapshots: [WorkspaceSnapshot],
        validProjectIDs: Set<UUID>
    ) -> [RestoredWorkspace] {
        var results: [RestoredWorkspace] = []
        for snapshot in snapshots {
            guard validProjectIDs.contains(snapshot.projectID) else { continue }
            let root = restoreSplitNode(from: snapshot.root)
            let areas = root.allAreas()
            guard !areas.isEmpty else { continue }
            let focusedID: UUID = if let areaID = snapshot.focusedAreaID, root.findArea(id: areaID) != nil {
                areaID
            } else {
                areas[0].id
            }
            results.append(RestoredWorkspace(projectID: snapshot.projectID, root: root, focusedAreaID: focusedID))
        }
        return results
    }

    static func snapshotAll(
        workspaceRoots: [UUID: SplitNode],
        focusedAreaID: [UUID: UUID]
    ) -> [WorkspaceSnapshot] {
        var snapshots: [WorkspaceSnapshot] = []
        for (projectID, root) in workspaceRoots {
            snapshots.append(WorkspaceSnapshot(
                projectID: projectID,
                focusedAreaID: focusedAreaID[projectID],
                root: snapshotSplitNode(root)
            ))
        }
        return snapshots
    }

    private static func restoreSplitNode(from snapshot: SplitNodeSnapshot) -> SplitNode {
        switch snapshot {
        case let .tabArea(areaSnapshot):
            return .tabArea(TabArea(restoring: areaSnapshot))
        case let .split(branchSnapshot):
            let first = restoreSplitNode(from: branchSnapshot.first)
            let second = restoreSplitNode(from: branchSnapshot.second)
            let direction: SplitDirection = switch branchSnapshot.direction {
            case .horizontal: .horizontal
            case .vertical: .vertical
            }
            return .split(SplitBranch(
                direction: direction,
                ratio: CGFloat(branchSnapshot.ratio),
                first: first,
                second: second
            ))
        }
    }

    private static func snapshotSplitNode(_ node: SplitNode) -> SplitNodeSnapshot {
        switch node {
        case let .tabArea(area):
            return .tabArea(area.snapshot())
        case let .split(branch):
            let direction: SplitDirectionSnapshot = switch branch.direction {
            case .horizontal: .horizontal
            case .vertical: .vertical
            }
            return .split(SplitBranchSnapshot(
                direction: direction,
                ratio: Double(branch.ratio),
                first: snapshotSplitNode(branch.first),
                second: snapshotSplitNode(branch.second)
            ))
        }
    }
}
