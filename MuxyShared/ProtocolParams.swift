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
    public let bytes: Data
    public init(paneID: UUID, bytes: Data) {
        self.paneID = paneID
        self.bytes = bytes
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

public struct RegisterDeviceParams: Codable, Sendable {
    public let deviceName: String
    public init(deviceName: String) {
        self.deviceName = deviceName
    }
}

public struct PairDeviceParams: Codable, Sendable {
    public let deviceID: UUID
    public let deviceName: String
    public let token: String
    public init(deviceID: UUID, deviceName: String, token: String) {
        self.deviceID = deviceID
        self.deviceName = deviceName
        self.token = token
    }
}

public struct AuthenticateDeviceParams: Codable, Sendable {
    public let deviceID: UUID
    public let deviceName: String
    public let token: String
    public init(deviceID: UUID, deviceName: String, token: String) {
        self.deviceID = deviceID
        self.deviceName = deviceName
        self.token = token
    }
}

public struct PairingResultDTO: Codable, Sendable {
    public let clientID: UUID
    public let deviceName: String
    public let themeFg: UInt32?
    public let themeBg: UInt32?
    public let themePalette: [UInt32]?
    public init(clientID: UUID, deviceName: String, themeFg: UInt32? = nil, themeBg: UInt32? = nil, themePalette: [UInt32]? = nil) {
        self.clientID = clientID
        self.deviceName = deviceName
        self.themeFg = themeFg
        self.themeBg = themeBg
        self.themePalette = themePalette
    }
}

public struct DeviceInfoDTO: Codable, Sendable {
    public let clientID: UUID
    public let deviceName: String
    public let themeFg: UInt32?
    public let themeBg: UInt32?
    public let themePalette: [UInt32]?
    public init(clientID: UUID, deviceName: String, themeFg: UInt32? = nil, themeBg: UInt32? = nil, themePalette: [UInt32]? = nil) {
        self.clientID = clientID
        self.deviceName = deviceName
        self.themeFg = themeFg
        self.themeBg = themeBg
        self.themePalette = themePalette
    }
}

public enum PaneOwnerDTO: Codable, Sendable, Equatable {
    case mac(deviceName: String)
    case remote(deviceID: UUID, deviceName: String)

    public var displayName: String {
        switch self {
        case let .mac(name): name
        case let .remote(_, name): name
        }
    }
}

public struct TakeOverPaneParams: Codable, Sendable {
    public let paneID: UUID
    public let cols: UInt32
    public let rows: UInt32
    public init(paneID: UUID, cols: UInt32, rows: UInt32) {
        self.paneID = paneID
        self.cols = cols
        self.rows = rows
    }
}

public struct ReleasePaneParams: Codable, Sendable {
    public let paneID: UUID
    public init(paneID: UUID) {
        self.paneID = paneID
    }
}

public struct PaneOwnershipEventDTO: Codable, Sendable {
    public let paneID: UUID
    public let owner: PaneOwnerDTO
    public init(paneID: UUID, owner: PaneOwnerDTO) {
        self.paneID = paneID
        self.owner = owner
    }
}

public struct DeviceThemeEventDTO: Codable, Sendable {
    public let fg: UInt32
    public let bg: UInt32
    public let palette: [UInt32]?
    public init(fg: UInt32, bg: UInt32, palette: [UInt32]? = nil) {
        self.fg = fg
        self.bg = bg
        self.palette = palette
    }
}

public struct TerminalScrollParams: Codable, Sendable {
    public let paneID: UUID
    public let deltaX: Double
    public let deltaY: Double
    public let precise: Bool
    public init(paneID: UUID, deltaX: Double, deltaY: Double, precise: Bool) {
        self.paneID = paneID
        self.deltaX = deltaX
        self.deltaY = deltaY
        self.precise = precise
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

public struct TerminalCellDTO: Codable, Sendable {
    public let codepoint: UInt32
    public let fg: UInt32
    public let bg: UInt32
    public let flags: UInt16

    public init(codepoint: UInt32, fg: UInt32, bg: UInt32, flags: UInt16) {
        self.codepoint = codepoint
        self.fg = fg
        self.bg = bg
        self.flags = flags
    }
}

public struct TerminalCellsDTO: Codable, Sendable {
    public let paneID: UUID
    public let cols: UInt32
    public let rows: UInt32
    public let cursorX: UInt32
    public let cursorY: UInt32
    public let cursorVisible: Bool
    public let defaultFg: UInt32
    public let defaultBg: UInt32
    public let cells: [TerminalCellDTO]

    public init(
        paneID: UUID,
        cols: UInt32,
        rows: UInt32,
        cursorX: UInt32,
        cursorY: UInt32,
        cursorVisible: Bool,
        defaultFg: UInt32,
        defaultBg: UInt32,
        cells: [TerminalCellDTO]
    ) {
        self.paneID = paneID
        self.cols = cols
        self.rows = rows
        self.cursorX = cursorX
        self.cursorY = cursorY
        self.cursorVisible = cursorVisible
        self.defaultFg = defaultFg
        self.defaultBg = defaultBg
        self.cells = cells
    }
}

public enum TerminalCellFlag {
    public static let bold: UInt16 = 1 << 0
    public static let italic: UInt16 = 1 << 1
    public static let faint: UInt16 = 1 << 2
    public static let blink: UInt16 = 1 << 3
    public static let inverse: UInt16 = 1 << 4
    public static let invisible: UInt16 = 1 << 5
    public static let strike: UInt16 = 1 << 6
    public static let underline: UInt16 = 1 << 7
    public static let overline: UInt16 = 1 << 8
    public static let wide: UInt16 = 1 << 9
    public static let spacer: UInt16 = 1 << 10
}

public struct TerminalOutputEventDTO: Codable, Sendable {
    public let paneID: UUID
    public let bytes: Data
    public init(paneID: UUID, bytes: Data) {
        self.paneID = paneID
        self.bytes = bytes
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

public struct VCSStageFilesParams: Codable, Sendable {
    public let projectID: UUID
    public let paths: [String]
    public init(projectID: UUID, paths: [String]) {
        self.projectID = projectID
        self.paths = paths
    }
}

public struct VCSUnstageFilesParams: Codable, Sendable {
    public let projectID: UUID
    public let paths: [String]
    public init(projectID: UUID, paths: [String]) {
        self.projectID = projectID
        self.paths = paths
    }
}

public struct VCSDiscardFilesParams: Codable, Sendable {
    public let projectID: UUID
    public let paths: [String]
    public let untrackedPaths: [String]
    public init(projectID: UUID, paths: [String], untrackedPaths: [String]) {
        self.projectID = projectID
        self.paths = paths
        self.untrackedPaths = untrackedPaths
    }
}

public struct VCSListBranchesParams: Codable, Sendable {
    public let projectID: UUID
    public init(projectID: UUID) {
        self.projectID = projectID
    }
}

public struct VCSSwitchBranchParams: Codable, Sendable {
    public let projectID: UUID
    public let branch: String
    public init(projectID: UUID, branch: String) {
        self.projectID = projectID
        self.branch = branch
    }
}

public struct VCSCreateBranchParams: Codable, Sendable {
    public let projectID: UUID
    public let name: String
    public init(projectID: UUID, name: String) {
        self.projectID = projectID
        self.name = name
    }
}

public struct VCSCreatePRParams: Codable, Sendable {
    public let projectID: UUID
    public let title: String
    public let body: String
    public let baseBranch: String?
    public let draft: Bool
    public init(projectID: UUID, title: String, body: String, baseBranch: String?, draft: Bool) {
        self.projectID = projectID
        self.title = title
        self.body = body
        self.baseBranch = baseBranch
        self.draft = draft
    }
}

public struct VCSAddWorktreeParams: Codable, Sendable {
    public let projectID: UUID
    public let name: String
    public let branch: String
    public let createBranch: Bool
    public init(projectID: UUID, name: String, branch: String, createBranch: Bool) {
        self.projectID = projectID
        self.name = name
        self.branch = branch
        self.createBranch = createBranch
    }
}

public struct VCSRemoveWorktreeParams: Codable, Sendable {
    public let projectID: UUID
    public let worktreeID: UUID
    public init(projectID: UUID, worktreeID: UUID) {
        self.projectID = projectID
        self.worktreeID = worktreeID
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
