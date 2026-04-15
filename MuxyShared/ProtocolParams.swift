import Foundation

public struct SelectProjectParams: Codable, Sendable {
    public let projectID: UUID
    public init(projectID: UUID) {
        self.projectID = projectID
    }
}

public struct ListWorktreesParams: Codable, Sendable {
    public let projectID: UUID
    public init(projectID: UUID) {
        self.projectID = projectID
    }
}

public struct SelectWorktreeParams: Codable, Sendable {
    public let projectID: UUID
    public let worktreeID: UUID
    public init(projectID: UUID, worktreeID: UUID) {
        self.projectID = projectID
        self.worktreeID = worktreeID
    }
}

public struct GetWorkspaceParams: Codable, Sendable {
    public let projectID: UUID
    public init(projectID: UUID) {
        self.projectID = projectID
    }
}

public struct CreateTabParams: Codable, Sendable {
    public let projectID: UUID
    public let areaID: UUID?
    public let kind: TabKindDTO
    public init(projectID: UUID, areaID: UUID? = nil, kind: TabKindDTO = .terminal) {
        self.projectID = projectID
        self.areaID = areaID
        self.kind = kind
    }
}

public struct CloseTabParams: Codable, Sendable {
    public let projectID: UUID
    public let areaID: UUID
    public let tabID: UUID
    public init(projectID: UUID, areaID: UUID, tabID: UUID) {
        self.projectID = projectID
        self.areaID = areaID
        self.tabID = tabID
    }
}

public struct SelectTabParams: Codable, Sendable {
    public let projectID: UUID
    public let areaID: UUID
    public let tabID: UUID
    public init(projectID: UUID, areaID: UUID, tabID: UUID) {
        self.projectID = projectID
        self.areaID = areaID
        self.tabID = tabID
    }
}

public struct SplitAreaParams: Codable, Sendable {
    public let projectID: UUID
    public let areaID: UUID
    public let direction: SplitDirectionDTO
    public let position: SplitPositionDTO
    public init(projectID: UUID, areaID: UUID, direction: SplitDirectionDTO, position: SplitPositionDTO) {
        self.projectID = projectID
        self.areaID = areaID
        self.direction = direction
        self.position = position
    }
}

public enum SplitPositionDTO: String, Codable, Sendable {
    case first
    case second
}

public struct CloseAreaParams: Codable, Sendable {
    public let projectID: UUID
    public let areaID: UUID
    public init(projectID: UUID, areaID: UUID) {
        self.projectID = projectID
        self.areaID = areaID
    }
}

public struct FocusAreaParams: Codable, Sendable {
    public let projectID: UUID
    public let areaID: UUID
    public init(projectID: UUID, areaID: UUID) {
        self.projectID = projectID
        self.areaID = areaID
    }
}

public struct TerminalInputParams: Codable, Sendable {
    public let paneID: UUID
    public let text: String
    public init(paneID: UUID, text: String) {
        self.paneID = paneID
        self.text = text
    }
}

public struct TerminalResizeParams: Codable, Sendable {
    public let paneID: UUID
    public let cols: UInt32
    public let rows: UInt32
    public init(paneID: UUID, cols: UInt32, rows: UInt32) {
        self.paneID = paneID
        self.cols = cols
        self.rows = rows
    }
}

public struct GetTerminalContentParams: Codable, Sendable {
    public let paneID: UUID
    public init(paneID: UUID) {
        self.paneID = paneID
    }
}

public struct TerminalContentDTO: Codable, Sendable {
    public let paneID: UUID
    public let content: String
    public let cols: UInt32
    public let rows: UInt32

    public init(paneID: UUID, content: String, cols: UInt32, rows: UInt32) {
        self.paneID = paneID
        self.content = content
        self.cols = cols
        self.rows = rows
    }
}

public struct TerminalOutputEventDTO: Codable, Sendable {
    public let paneID: UUID
    public let data: String
    public init(paneID: UUID, data: String) {
        self.paneID = paneID
        self.data = data
    }
}

public struct TabChangeEventDTO: Codable, Sendable {
    public let projectID: UUID
    public let areaID: UUID
    public let tab: TabDTO
    public let changeKind: TabChangeKind
    public init(projectID: UUID, areaID: UUID, tab: TabDTO, changeKind: TabChangeKind) {
        self.projectID = projectID
        self.areaID = areaID
        self.tab = tab
        self.changeKind = changeKind
    }

    public enum TabChangeKind: String, Codable, Sendable {
        case created
        case closed
        case selected
        case titleChanged
    }
}

public struct GetVCSStatusParams: Codable, Sendable {
    public let projectID: UUID
    public init(projectID: UUID) {
        self.projectID = projectID
    }
}

public struct VCSCommitParams: Codable, Sendable {
    public let projectID: UUID
    public let message: String
    public let stageAll: Bool
    public init(projectID: UUID, message: String, stageAll: Bool = false) {
        self.projectID = projectID
        self.message = message
        self.stageAll = stageAll
    }
}

public struct VCSPushParams: Codable, Sendable {
    public let projectID: UUID
    public init(projectID: UUID) {
        self.projectID = projectID
    }
}

public struct VCSPullParams: Codable, Sendable {
    public let projectID: UUID
    public init(projectID: UUID) {
        self.projectID = projectID
    }
}

public struct GetProjectLogoParams: Codable, Sendable {
    public let projectID: UUID
    public init(projectID: UUID) {
        self.projectID = projectID
    }
}

public struct ProjectLogoDTO: Codable, Sendable {
    public let projectID: UUID
    public let pngData: String

    public init(projectID: UUID, pngData: String) {
        self.projectID = projectID
        self.pngData = pngData
    }
}

public struct MarkNotificationReadParams: Codable, Sendable {
    public let notificationID: UUID
    public init(notificationID: UUID) {
        self.notificationID = notificationID
    }
}

public struct SubscribeParams: Codable, Sendable {
    public let events: [MuxyEventKind]
    public init(events: [MuxyEventKind]) {
        self.events = events
    }
}

public struct UnsubscribeParams: Codable, Sendable {
    public let events: [MuxyEventKind]
    public init(events: [MuxyEventKind]) {
        self.events = events
    }
}
