import Foundation

struct WorkspaceSnapshot: Codable {
    let projectID: UUID
    let worktreeID: UUID?
    let worktreePath: String?
    let focusedAreaID: UUID?
    let root: SplitNodeSnapshot

    init(
        projectID: UUID,
        worktreeID: UUID?,
        worktreePath: String?,
        focusedAreaID: UUID?,
        root: SplitNodeSnapshot
    ) {
        self.projectID = projectID
        self.worktreeID = worktreeID
        self.worktreePath = worktreePath
        self.focusedAreaID = focusedAreaID
        self.root = root
    }

    private enum CodingKeys: String, CodingKey {
        case projectID
        case worktreeID
        case worktreePath
        case focusedAreaID
        case root
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        projectID = try container.decode(UUID.self, forKey: .projectID)
        worktreeID = try container.decodeIfPresent(UUID.self, forKey: .worktreeID)
        worktreePath = try container.decodeIfPresent(String.self, forKey: .worktreePath)
        focusedAreaID = try container.decodeIfPresent(UUID.self, forKey: .focusedAreaID)
        root = try container.decode(SplitNodeSnapshot.self, forKey: .root)
    }
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
    let kind: TerminalTab.Kind
    let id: UUID
    let customTitle: String?
    let colorID: String?
    let isPinned: Bool
    let projectPath: String
    let paneTitle: String
    let paneID: UUID?
    let filePath: String?
    let currentWorkingDirectory: String?

    init(
        kind: TerminalTab.Kind,
        id: UUID = UUID(),
        customTitle: String?,
        colorID: String?,
        isPinned: Bool,
        projectPath: String,
        paneTitle: String?,
        paneID: UUID? = nil,
        filePath: String? = nil,
        currentWorkingDirectory: String? = nil
    ) {
        self.kind = kind
        self.id = id
        self.customTitle = customTitle
        self.colorID = colorID
        self.isPinned = isPinned
        self.projectPath = projectPath
        self.paneTitle = paneTitle ?? "Terminal"
        self.paneID = paneID
        self.filePath = filePath
        self.currentWorkingDirectory = currentWorkingDirectory
    }

    private enum CodingKeys: String, CodingKey {
        case kind
        case id
        case customTitle
        case colorID
        case isPinned
        case projectPath
        case paneTitle
        case paneID
        case filePath
        case currentWorkingDirectory
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        kind = try container.decodeIfPresent(TerminalTab.Kind.self, forKey: .kind) ?? .terminal
        id = try container.decodeIfPresent(UUID.self, forKey: .id) ?? UUID()
        customTitle = try container.decodeIfPresent(String.self, forKey: .customTitle)
        colorID = try container.decodeIfPresent(String.self, forKey: .colorID)
        isPinned = try container.decode(Bool.self, forKey: .isPinned)
        projectPath = try container.decode(String.self, forKey: .projectPath)
        paneTitle = try container.decodeIfPresent(String.self, forKey: .paneTitle) ?? "Terminal"
        paneID = try container.decodeIfPresent(UUID.self, forKey: .paneID)
        filePath = try container.decodeIfPresent(String.self, forKey: .filePath)
        currentWorkingDirectory = try container.decodeIfPresent(String.self, forKey: .currentWorkingDirectory)
    }
}

struct RestoredWorkspace {
    let key: WorktreeKey
    let root: SplitNode
    let focusedAreaID: UUID
}

@MainActor
enum WorkspaceRestorer {
    static func restoreAll(
        from snapshots: [WorkspaceSnapshot],
        projects: [Project],
        worktrees: [UUID: [Worktree]],
        sessionsByPaneID: [UUID: TerminalSessionSnapshot] = [:]
    ) -> [RestoredWorkspace] {
        let projectByID = Dictionary(uniqueKeysWithValues: projects.map { ($0.id, $0) })
        var results: [RestoredWorkspace] = []
        for snapshot in snapshots {
            guard projectByID[snapshot.projectID] != nil else { continue }
            let worktreeList = worktrees[snapshot.projectID] ?? []
            guard let targetWorktree = resolveWorktree(for: snapshot, in: worktreeList) else { continue }
            let root = restoreSplitNode(from: snapshot.root, sessionsByPaneID: sessionsByPaneID)
            let areas = root.allAreas()
            guard !areas.isEmpty else { continue }
            let focusedID: UUID = if let areaID = snapshot.focusedAreaID, root.findArea(id: areaID) != nil {
                areaID
            } else {
                areas[0].id
            }
            let key = WorktreeKey(projectID: snapshot.projectID, worktreeID: targetWorktree.id)
            results.append(RestoredWorkspace(key: key, root: root, focusedAreaID: focusedID))
        }
        return results
    }

    private static func resolveWorktree(for snapshot: WorkspaceSnapshot, in worktrees: [Worktree]) -> Worktree? {
        if let worktreeID = snapshot.worktreeID,
           let match = worktrees.first(where: { $0.id == worktreeID })
        {
            return match
        }
        if let worktreePath = snapshot.worktreePath,
           let match = worktrees.first(where: { $0.path == worktreePath })
        {
            return match
        }
        return worktrees.first(where: { $0.isPrimary }) ?? worktrees.first
    }

    static func snapshotAll(
        workspaceRoots: [WorktreeKey: SplitNode],
        focusedAreaID: [WorktreeKey: UUID]
    ) -> [WorkspaceSnapshot] {
        var snapshots: [WorkspaceSnapshot] = []
        for (key, root) in workspaceRoots {
            let path: String? = {
                if case let .tabArea(area) = root { return area.projectPath }
                return root.allAreas().first?.projectPath
            }()
            snapshots.append(WorkspaceSnapshot(
                projectID: key.projectID,
                worktreeID: key.worktreeID,
                worktreePath: path,
                focusedAreaID: focusedAreaID[key],
                root: snapshotSplitNode(root)
            ))
        }
        return snapshots
    }

    private static func restoreSplitNode(
        from snapshot: SplitNodeSnapshot,
        sessionsByPaneID: [UUID: TerminalSessionSnapshot]
    ) -> SplitNode {
        switch snapshot {
        case let .tabArea(areaSnapshot):
            return .tabArea(TabArea(restoring: areaSnapshot, sessionsByPaneID: sessionsByPaneID))
        case let .split(branchSnapshot):
            let first = restoreSplitNode(from: branchSnapshot.first, sessionsByPaneID: sessionsByPaneID)
            let second = restoreSplitNode(from: branchSnapshot.second, sessionsByPaneID: sessionsByPaneID)
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
