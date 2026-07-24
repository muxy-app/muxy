import Foundation

struct WorkspaceSnapshot: Codable {
    let projectID: UUID
    let worktreeID: UUID?
    let worktreePath: String?
    let focusedAreaID: UUID?
    let topLevelTabOrder: [UUID]?
    let topLevelTabLayout: TopLevelTabNodeSnapshot?
    let root: SplitNodeSnapshot

    init(
        projectID: UUID,
        worktreeID: UUID?,
        worktreePath: String?,
        focusedAreaID: UUID?,
        topLevelTabOrder: [UUID]? = nil,
        topLevelTabLayout: TopLevelTabNodeSnapshot? = nil,
        root: SplitNodeSnapshot
    ) {
        self.projectID = projectID
        self.worktreeID = worktreeID
        self.worktreePath = worktreePath
        self.focusedAreaID = focusedAreaID
        self.topLevelTabOrder = topLevelTabOrder
        self.topLevelTabLayout = topLevelTabLayout
        self.root = root
    }

    private enum CodingKeys: String, CodingKey {
        case projectID
        case worktreeID
        case worktreePath
        case focusedAreaID
        case topLevelTabOrder
        case topLevelTabLayout
        case root
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        projectID = try container.decode(UUID.self, forKey: .projectID)
        worktreeID = try container.decodeIfPresent(UUID.self, forKey: .worktreeID)
        worktreePath = try container.decodeIfPresent(String.self, forKey: .worktreePath)
        focusedAreaID = try container.decodeIfPresent(UUID.self, forKey: .focusedAreaID)
        topLevelTabOrder = try container.decodeIfPresent([UUID].self, forKey: .topLevelTabOrder)
        topLevelTabLayout = try container.decodeIfPresent(TopLevelTabNodeSnapshot.self, forKey: .topLevelTabLayout)
        root = try container.decode(SplitNodeSnapshot.self, forKey: .root)
    }
}

indirect enum TopLevelTabNodeSnapshot: Codable {
    case group(TopLevelTabGroupSnapshot)
    case split(TopLevelTabBranchSnapshot)

    private enum CodingKeys: String, CodingKey {
        case type
        case group
        case split
    }

    private enum NodeType: String, Codable {
        case group
        case split
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(NodeType.self, forKey: .type) {
        case .group:
            self = try .group(container.decode(TopLevelTabGroupSnapshot.self, forKey: .group))
        case .split:
            self = try .split(container.decode(TopLevelTabBranchSnapshot.self, forKey: .split))
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .group(group):
            try container.encode(NodeType.group, forKey: .type)
            try container.encode(group, forKey: .group)
        case let .split(branch):
            try container.encode(NodeType.split, forKey: .type)
            try container.encode(branch, forKey: .split)
        }
    }
}

struct TopLevelTabGroupSnapshot: Codable {
    let tabIDs: [UUID]
    let activeTabID: UUID?
}

struct TopLevelTabBranchSnapshot: Codable {
    let direction: SplitDirectionSnapshot
    let ratio: Double
    let first: TopLevelTabNodeSnapshot
    let second: TopLevelTabNodeSnapshot
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
    let parentTabID: UUID?
    let customTitle: String?
    let colorID: String?
    let customIcon: String?
    let isPinned: Bool
    let projectPath: String
    let paneTitle: String
    let paneID: UUID?
    let filePath: String?
    let currentWorkingDirectory: String?
    let extensionID: String?
    let extensionTabTypeID: String?
    let extensionTabData: ExtensionJSON?
    let browserURL: String?
    let browserProfileID: String?

    init(
        kind: TerminalTab.Kind,
        id: UUID = UUID(),
        parentTabID: UUID? = nil,
        customTitle: String?,
        colorID: String?,
        customIcon: String? = nil,
        isPinned: Bool,
        projectPath: String,
        paneTitle: String?,
        paneID: UUID? = nil,
        filePath: String? = nil,
        currentWorkingDirectory: String? = nil,
        extensionID: String? = nil,
        extensionTabTypeID: String? = nil,
        extensionTabData: ExtensionJSON? = nil,
        browserURL: String? = nil,
        browserProfileID: String? = nil
    ) {
        self.kind = kind
        self.id = id
        self.parentTabID = parentTabID
        self.customTitle = customTitle
        self.colorID = colorID
        self.customIcon = customIcon
        self.isPinned = isPinned
        self.projectPath = projectPath
        self.paneTitle = paneTitle ?? "Terminal"
        self.paneID = paneID
        self.filePath = filePath
        self.currentWorkingDirectory = currentWorkingDirectory
        self.extensionID = extensionID
        self.extensionTabTypeID = extensionTabTypeID
        self.extensionTabData = extensionTabData
        self.browserURL = browserURL
        self.browserProfileID = browserProfileID
    }

    private enum CodingKeys: String, CodingKey {
        case kind
        case id
        case parentTabID
        case customTitle
        case colorID
        case customIcon
        case isPinned
        case projectPath
        case paneTitle
        case paneID
        case filePath
        case currentWorkingDirectory
        case extensionID
        case extensionTabTypeID
        case extensionTabData
        case browserURL
        case browserProfileID
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let rawKind = try container.decodeIfPresent(String.self, forKey: .kind)
        kind = rawKind.flatMap(TerminalTab.Kind.init(rawValue:)) ?? .terminal
        id = try container.decodeIfPresent(UUID.self, forKey: .id) ?? UUID()
        parentTabID = try container.decodeIfPresent(UUID.self, forKey: .parentTabID)
        customTitle = try container.decodeIfPresent(String.self, forKey: .customTitle)
        colorID = try container.decodeIfPresent(String.self, forKey: .colorID)
        customIcon = try container.decodeIfPresent(String.self, forKey: .customIcon)
        isPinned = try container.decode(Bool.self, forKey: .isPinned)
        projectPath = try container.decode(String.self, forKey: .projectPath)
        paneTitle = try container.decodeIfPresent(String.self, forKey: .paneTitle) ?? "Terminal"
        paneID = try container.decodeIfPresent(UUID.self, forKey: .paneID)
        filePath = try container.decodeIfPresent(String.self, forKey: .filePath)
        currentWorkingDirectory = try container.decodeIfPresent(String.self, forKey: .currentWorkingDirectory)
        extensionID = try container.decodeIfPresent(String.self, forKey: .extensionID)
        extensionTabTypeID = try container.decodeIfPresent(String.self, forKey: .extensionTabTypeID)
        extensionTabData = try container.decodeIfPresent(ExtensionJSON.self, forKey: .extensionTabData)
        browserURL = try container.decodeIfPresent(String.self, forKey: .browserURL)
        browserProfileID = try container.decodeIfPresent(String.self, forKey: .browserProfileID)
    }
}

struct RestoredWorkspace {
    let key: WorktreeKey
    let root: SplitNode
    let focusedAreaID: UUID
    let topLevelTabOrder: [UUID]
    let topLevelTabLayout: TopLevelTabNode
}

@MainActor
enum WorkspaceRestorer {
    static func restoreAll(
        from snapshots: [WorkspaceSnapshot],
        projects: [Project],
        worktrees: [UUID: [Worktree]]
    ) -> [RestoredWorkspace] {
        let projectByID = Dictionary(uniqueKeysWithValues: projects.map { ($0.id, $0) })
        var results: [RestoredWorkspace] = []
        for snapshot in snapshots {
            guard projectByID[snapshot.projectID] != nil else { continue }
            let worktreeList = worktrees[snapshot.projectID] ?? []
            guard let targetWorktree = resolveWorktree(for: snapshot, in: worktreeList) else { continue }
            let root = restoreSplitNode(from: snapshot.root)
            let areas = root.allAreas()
            guard !areas.isEmpty else { continue }
            let focusedID: UUID = if let areaID = snapshot.focusedAreaID, root.findArea(id: areaID) != nil {
                areaID
            } else {
                areas[0].id
            }
            let key = WorktreeKey(projectID: snapshot.projectID, worktreeID: targetWorktree.id)
            let restoredTabs = root.allTabs()
            let validParentIDs = Set(restoredTabs.filter { $0.parentTabID == nil }.map(\.id))
            for tab in restoredTabs {
                guard let parentTabID = tab.parentTabID,
                      !validParentIDs.contains(parentTabID)
                else { continue }
                tab.parentTabID = nil
            }
            let rootTabIDs = restoredTabs.filter { $0.parentTabID == nil }.map(\.id)
            let persistedOrder = snapshot.topLevelTabOrder ?? []
            let validIDs = Set(rootTabIDs)
            var seenIDs = Set<UUID>()
            var ordered = persistedOrder.filter {
                validIDs.contains($0) && seenIDs.insert($0).inserted
            }
            ordered.append(contentsOf: rootTabIDs.filter {
                seenIDs.insert($0).inserted
            })
            let focusedTopLevelTabID = root.findArea(id: focusedID)?.activeTab.map {
                $0.parentTabID ?? $0.id
            }
            let restoredTopLevelLayout = snapshot.topLevelTabLayout
                .map(restoreTopLevelTabNode)
                .flatMap { $0.pruningTabs(validTabIDs: Set(ordered)) }
                ?? .group(TopLevelTabGroup(
                    tabIDs: ordered,
                    activeTabID: focusedTopLevelTabID ?? ordered.first
                ))
            let assignedIDs = Set(restoredTopLevelLayout.flattenedTabIDs())
            let destinationGroup = focusedTopLevelTabID
                .flatMap(restoredTopLevelLayout.group(containingTabID:))
                ?? restoredTopLevelLayout.allGroups()[0]
            destinationGroup.tabIDs.append(contentsOf: ordered.filter { !assignedIDs.contains($0) })
            if let focusedTopLevelTabID,
               destinationGroup.tabIDs.contains(focusedTopLevelTabID)
            {
                destinationGroup.activeTabID = focusedTopLevelTabID
            }
            results.append(RestoredWorkspace(
                key: key,
                root: root,
                focusedAreaID: focusedID,
                topLevelTabOrder: ordered,
                topLevelTabLayout: restoredTopLevelLayout
            ))
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
        focusedAreaID: [WorktreeKey: UUID],
        topLevelTabOrder: [WorktreeKey: [UUID]] = [:],
        topLevelTabLayouts: [WorktreeKey: TopLevelTabNode] = [:]
    ) -> [WorkspaceSnapshot] {
        var snapshots: [WorkspaceSnapshot] = []
        for (key, root) in workspaceRoots {
            let path: String? = {
                if case let .tabArea(area) = root {
                    return area.projectPath
                }
                return root.allAreas().first?.projectPath
            }()
            snapshots.append(WorkspaceSnapshot(
                projectID: key.projectID,
                worktreeID: key.worktreeID,
                worktreePath: path,
                focusedAreaID: focusedAreaID[key],
                topLevelTabOrder: topLevelTabOrder[key],
                topLevelTabLayout: topLevelTabLayouts[key].map(snapshotTopLevelTabNode),
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

    private static func restoreTopLevelTabNode(
        from snapshot: TopLevelTabNodeSnapshot
    ) -> TopLevelTabNode {
        switch snapshot {
        case let .group(group):
            return .group(TopLevelTabGroup(
                tabIDs: group.tabIDs,
                activeTabID: group.activeTabID
            ))
        case let .split(branch):
            let direction: SplitDirection = switch branch.direction {
            case .horizontal:
                .horizontal
            case .vertical:
                .vertical
            }
            return .split(TopLevelTabBranch(
                direction: direction,
                ratio: CGFloat(branch.ratio),
                first: restoreTopLevelTabNode(from: branch.first),
                second: restoreTopLevelTabNode(from: branch.second)
            ))
        }
    }

    private static func snapshotTopLevelTabNode(
        _ node: TopLevelTabNode
    ) -> TopLevelTabNodeSnapshot {
        switch node {
        case let .group(group):
            return .group(TopLevelTabGroupSnapshot(
                tabIDs: group.tabIDs,
                activeTabID: group.activeTabID
            ))
        case let .split(branch):
            let direction: SplitDirectionSnapshot = switch branch.direction {
            case .horizontal:
                .horizontal
            case .vertical:
                .vertical
            }
            return .split(TopLevelTabBranchSnapshot(
                direction: direction,
                ratio: Double(branch.ratio),
                first: snapshotTopLevelTabNode(branch.first),
                second: snapshotTopLevelTabNode(branch.second)
            ))
        }
    }
}
