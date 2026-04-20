import Foundation

public struct WorkspaceDTO: Codable, Sendable {
    public let projectID: UUID
    public let worktreeID: UUID
    public let focusedAreaID: UUID?
    public let root: SplitNodeDTO

    public init(
        projectID: UUID,
        worktreeID: UUID,
        focusedAreaID: UUID?,
        root: SplitNodeDTO
    ) {
        self.projectID = projectID
        self.worktreeID = worktreeID
        self.focusedAreaID = focusedAreaID
        self.root = root
    }
}

public enum SplitDirectionDTO: String, Codable, Sendable {
    case horizontal
    case vertical
}

public indirect enum SplitNodeDTO: Codable, Sendable {
    case tabArea(TabAreaDTO)
    case split(SplitBranchDTO)

    private enum CodingKeys: String, CodingKey {
        case type
        case tabArea
        case split
    }

    private enum NodeType: String, Codable {
        case tabArea
        case split
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(NodeType.self, forKey: .type)
        switch type {
        case .tabArea:
            self = try .tabArea(container.decode(TabAreaDTO.self, forKey: .tabArea))
        case .split:
            self = try .split(container.decode(SplitBranchDTO.self, forKey: .split))
        }
    }

    public func encode(to encoder: Encoder) throws {
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

public struct SplitBranchDTO: Codable, Sendable {
    public let id: UUID
    public let direction: SplitDirectionDTO
    public let ratio: Double
    public let first: SplitNodeDTO
    public let second: SplitNodeDTO

    public init(
        id: UUID,
        direction: SplitDirectionDTO,
        ratio: Double,
        first: SplitNodeDTO,
        second: SplitNodeDTO
    ) {
        self.id = id
        self.direction = direction
        self.ratio = ratio
        self.first = first
        self.second = second
    }
}

public struct TabAreaDTO: Identifiable, Codable, Sendable {
    public let id: UUID
    public let projectPath: String
    public let tabs: [TabDTO]
    public let activeTabID: UUID?

    public init(
        id: UUID,
        projectPath: String,
        tabs: [TabDTO],
        activeTabID: UUID?
    ) {
        self.id = id
        self.projectPath = projectPath
        self.tabs = tabs
        self.activeTabID = activeTabID
    }
}

public struct TabDTO: Identifiable, Codable, Sendable {
    public let id: UUID
    public let kind: TabKindDTO
    public let title: String
    public let isPinned: Bool
    public let paneID: UUID?

    public init(
        id: UUID,
        kind: TabKindDTO,
        title: String,
        isPinned: Bool,
        paneID: UUID? = nil
    ) {
        self.id = id
        self.kind = kind
        self.title = title
        self.isPinned = isPinned
        self.paneID = paneID
    }
}

public enum TabKindDTO: String, Codable, Sendable {
    case terminal
    case vcs
    case editor
    case diffViewer
}
