import Foundation

public enum MuxyMessageType: String, Codable, Sendable {
    case request
    case response
    case event
}

public struct MuxyRequest: Codable, Sendable {
    public let id: String
    public let method: MuxyMethod
    public let params: MuxyParams?

    public init(id: String, method: MuxyMethod, params: MuxyParams? = nil) {
        self.id = id
        self.method = method
        self.params = params
    }
}

public struct MuxyResponse: Codable, Sendable {
    public let id: String
    public let result: MuxyResult?
    public let error: MuxyError?

    public init(id: String, result: MuxyResult? = nil, error: MuxyError? = nil) {
        self.id = id
        self.result = result
        self.error = error
    }
}

public struct MuxyEvent: Codable, Sendable {
    public let event: MuxyEventKind
    public let data: MuxyEventData

    public init(event: MuxyEventKind, data: MuxyEventData) {
        self.event = event
        self.data = data
    }
}

public struct MuxyError: Codable, Sendable {
    public let code: Int
    public let message: String

    public init(code: Int, message: String) {
        self.code = code
        self.message = message
    }

    public static let notFound = MuxyError(code: 404, message: "Not found")
    public static let invalidParams = MuxyError(code: 400, message: "Invalid parameters")
    public static let internalError = MuxyError(code: 500, message: "Internal error")
    public static let unauthorized = MuxyError(code: 401, message: "Authentication required")
    public static let pairingDenied = MuxyError(code: 403, message: "Pairing denied")
    public static let pairingTimeout = MuxyError(code: 408, message: "Pairing request timed out")
}

public enum MuxyMethod: String, Codable, Sendable {
    case listProjects
    case selectProject
    case listWorktrees
    case selectWorktree
    case getWorkspace
    case createTab
    case closeTab
    case selectTab
    case splitArea
    case closeArea
    case focusArea
    case terminalInput
    case terminalResize
    case terminalScroll
    case getTerminalContent
    case registerDevice
    case pairDevice
    case authenticateDevice
    case takeOverPane
    case releasePane
    case getVCSStatus
    case vcsRefresh
    case vcsCommit
    case vcsPush
    case vcsPull
    case vcsStageFiles
    case vcsUnstageFiles
    case vcsDiscardFiles
    case vcsListBranches
    case vcsSwitchBranch
    case vcsCreateBranch
    case vcsCreatePR
    case vcsAddWorktree
    case vcsRemoveWorktree
    case getProjectLogo
    case listNotifications
    case markNotificationRead
    case subscribe
    case unsubscribe
}

public enum MuxyParams: Codable, Sendable {
    case selectProject(SelectProjectParams)
    case listWorktrees(ListWorktreesParams)
    case selectWorktree(SelectWorktreeParams)
    case getWorkspace(GetWorkspaceParams)
    case createTab(CreateTabParams)
    case closeTab(CloseTabParams)
    case selectTab(SelectTabParams)
    case splitArea(SplitAreaParams)
    case closeArea(CloseAreaParams)
    case focusArea(FocusAreaParams)
    case terminalInput(TerminalInputParams)
    case terminalResize(TerminalResizeParams)
    case terminalScroll(TerminalScrollParams)
    case getTerminalContent(GetTerminalContentParams)
    case registerDevice(RegisterDeviceParams)
    case pairDevice(PairDeviceParams)
    case authenticateDevice(AuthenticateDeviceParams)
    case takeOverPane(TakeOverPaneParams)
    case releasePane(ReleasePaneParams)
    case getVCSStatus(GetVCSStatusParams)
    case vcsRefresh(VCSRefreshParams)
    case vcsCommit(VCSCommitParams)
    case vcsPush(VCSPushParams)
    case vcsPull(VCSPullParams)
    case vcsStageFiles(VCSStageFilesParams)
    case vcsUnstageFiles(VCSUnstageFilesParams)
    case vcsDiscardFiles(VCSDiscardFilesParams)
    case vcsListBranches(VCSListBranchesParams)
    case vcsSwitchBranch(VCSSwitchBranchParams)
    case vcsCreateBranch(VCSCreateBranchParams)
    case vcsCreatePR(VCSCreatePRParams)
    case vcsAddWorktree(VCSAddWorktreeParams)
    case vcsRemoveWorktree(VCSRemoveWorktreeParams)
    case getProjectLogo(GetProjectLogoParams)
    case markNotificationRead(MarkNotificationReadParams)
    case subscribe(SubscribeParams)
    case unsubscribe(UnsubscribeParams)

    private enum CodingKeys: String, CodingKey {
        case type
        case value
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)
        switch type {
        case "selectProject": self = try .selectProject(container.decode(SelectProjectParams.self, forKey: .value))
        case "listWorktrees": self = try .listWorktrees(container.decode(ListWorktreesParams.self, forKey: .value))
        case "selectWorktree": self = try .selectWorktree(container.decode(SelectWorktreeParams.self, forKey: .value))
        case "getWorkspace": self = try .getWorkspace(container.decode(GetWorkspaceParams.self, forKey: .value))
        case "createTab": self = try .createTab(container.decode(CreateTabParams.self, forKey: .value))
        case "closeTab": self = try .closeTab(container.decode(CloseTabParams.self, forKey: .value))
        case "selectTab": self = try .selectTab(container.decode(SelectTabParams.self, forKey: .value))
        case "splitArea": self = try .splitArea(container.decode(SplitAreaParams.self, forKey: .value))
        case "closeArea": self = try .closeArea(container.decode(CloseAreaParams.self, forKey: .value))
        case "focusArea": self = try .focusArea(container.decode(FocusAreaParams.self, forKey: .value))
        case "terminalInput": self = try .terminalInput(container.decode(TerminalInputParams.self, forKey: .value))
        case "terminalResize": self = try .terminalResize(container.decode(TerminalResizeParams.self, forKey: .value))
        case "terminalScroll": self = try .terminalScroll(container.decode(TerminalScrollParams.self, forKey: .value))
        case "registerDevice": self = try .registerDevice(container.decode(RegisterDeviceParams.self, forKey: .value))
        case "pairDevice": self = try .pairDevice(container.decode(PairDeviceParams.self, forKey: .value))
        case "authenticateDevice": self = try .authenticateDevice(container.decode(AuthenticateDeviceParams.self, forKey: .value))
        case "takeOverPane": self = try .takeOverPane(container.decode(TakeOverPaneParams.self, forKey: .value))
        case "releasePane": self = try .releasePane(container.decode(ReleasePaneParams.self, forKey: .value))
        case "getTerminalContent": self = try .getTerminalContent(container.decode(GetTerminalContentParams.self, forKey: .value))
        case "getVCSStatus": self = try .getVCSStatus(container.decode(GetVCSStatusParams.self, forKey: .value))
        case "vcsRefresh": self = try .vcsRefresh(container.decode(VCSRefreshParams.self, forKey: .value))
        case "vcsCommit": self = try .vcsCommit(container.decode(VCSCommitParams.self, forKey: .value))
        case "vcsPush": self = try .vcsPush(container.decode(VCSPushParams.self, forKey: .value))
        case "vcsPull": self = try .vcsPull(container.decode(VCSPullParams.self, forKey: .value))
        case "vcsStageFiles": self = try .vcsStageFiles(container.decode(VCSStageFilesParams.self, forKey: .value))
        case "vcsUnstageFiles": self = try .vcsUnstageFiles(container.decode(VCSUnstageFilesParams.self, forKey: .value))
        case "vcsDiscardFiles": self = try .vcsDiscardFiles(container.decode(VCSDiscardFilesParams.self, forKey: .value))
        case "vcsListBranches": self = try .vcsListBranches(container.decode(VCSListBranchesParams.self, forKey: .value))
        case "vcsSwitchBranch": self = try .vcsSwitchBranch(container.decode(VCSSwitchBranchParams.self, forKey: .value))
        case "vcsCreateBranch": self = try .vcsCreateBranch(container.decode(VCSCreateBranchParams.self, forKey: .value))
        case "vcsCreatePR": self = try .vcsCreatePR(container.decode(VCSCreatePRParams.self, forKey: .value))
        case "vcsAddWorktree": self = try .vcsAddWorktree(container.decode(VCSAddWorktreeParams.self, forKey: .value))
        case "vcsRemoveWorktree": self = try .vcsRemoveWorktree(container.decode(VCSRemoveWorktreeParams.self, forKey: .value))
        case "getProjectLogo": self = try .getProjectLogo(container.decode(GetProjectLogoParams.self, forKey: .value))
        case "markNotificationRead": self = try .markNotificationRead(container.decode(MarkNotificationReadParams.self, forKey: .value))
        case "subscribe": self = try .subscribe(container.decode(SubscribeParams.self, forKey: .value))
        case "unsubscribe": self = try .unsubscribe(container.decode(UnsubscribeParams.self, forKey: .value))
        default: throw DecodingError.dataCorruptedError(forKey: .type, in: container, debugDescription: "Unknown param type: \(type)")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .selectProject(v): try container.encode("selectProject", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .listWorktrees(v): try container.encode("listWorktrees", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .selectWorktree(v): try container.encode("selectWorktree", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .getWorkspace(v): try container.encode("getWorkspace", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .createTab(v): try container.encode("createTab", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .closeTab(v): try container.encode("closeTab", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .selectTab(v): try container.encode("selectTab", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .splitArea(v): try container.encode("splitArea", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .closeArea(v): try container.encode("closeArea", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .focusArea(v): try container.encode("focusArea", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .terminalInput(v): try container.encode("terminalInput", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .terminalResize(v): try container.encode("terminalResize", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .terminalScroll(v): try container.encode("terminalScroll", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .registerDevice(v): try container.encode("registerDevice", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .pairDevice(v): try container.encode("pairDevice", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .authenticateDevice(v): try container.encode("authenticateDevice", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .takeOverPane(v): try container.encode("takeOverPane", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .releasePane(v): try container.encode("releasePane", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .getTerminalContent(v): try container.encode("getTerminalContent", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .getVCSStatus(v): try container.encode("getVCSStatus", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .vcsRefresh(v): try container.encode("vcsRefresh", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .vcsCommit(v): try container.encode("vcsCommit", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .vcsPush(v): try container.encode("vcsPush", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .vcsPull(v): try container.encode("vcsPull", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .vcsStageFiles(v): try container.encode("vcsStageFiles", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .vcsUnstageFiles(v): try container.encode("vcsUnstageFiles", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .vcsDiscardFiles(v): try container.encode("vcsDiscardFiles", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .vcsListBranches(v): try container.encode("vcsListBranches", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .vcsSwitchBranch(v): try container.encode("vcsSwitchBranch", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .vcsCreateBranch(v): try container.encode("vcsCreateBranch", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .vcsCreatePR(v): try container.encode("vcsCreatePR", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .vcsAddWorktree(v): try container.encode("vcsAddWorktree", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .vcsRemoveWorktree(v): try container.encode("vcsRemoveWorktree", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .getProjectLogo(v): try container.encode("getProjectLogo", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .markNotificationRead(v): try container.encode("markNotificationRead", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .subscribe(v): try container.encode("subscribe", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .unsubscribe(v): try container.encode("unsubscribe", forKey: .type)
            try container.encode(v, forKey: .value)
        }
    }
}

public enum MuxyResult: Codable, Sendable {
    case projects([ProjectDTO])
    case worktrees([WorktreeDTO])
    case workspace(WorkspaceDTO)
    case tab(TabDTO)
    case terminalContent(TerminalContentDTO)
    case terminalCells(TerminalCellsDTO)
    case deviceInfo(DeviceInfoDTO)
    case pairing(PairingResultDTO)
    case paneOwner(PaneOwnerDTO)
    case vcsStatus(VCSStatusDTO)
    case vcsBranches(VCSBranchesDTO)
    case vcsPRCreated(VCSCreatePRResultDTO)
    case projectLogo(ProjectLogoDTO)
    case notifications([NotificationDTO])
    case ok

    private enum CodingKeys: String, CodingKey {
        case type
        case value
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)
        switch type {
        case "projects": self = try .projects(container.decode([ProjectDTO].self, forKey: .value))
        case "worktrees": self = try .worktrees(container.decode([WorktreeDTO].self, forKey: .value))
        case "workspace": self = try .workspace(container.decode(WorkspaceDTO.self, forKey: .value))
        case "tab": self = try .tab(container.decode(TabDTO.self, forKey: .value))
        case "terminalContent": self = try .terminalContent(container.decode(TerminalContentDTO.self, forKey: .value))
        case "terminalCells": self = try .terminalCells(container.decode(TerminalCellsDTO.self, forKey: .value))
        case "deviceInfo": self = try .deviceInfo(container.decode(DeviceInfoDTO.self, forKey: .value))
        case "pairing": self = try .pairing(container.decode(PairingResultDTO.self, forKey: .value))
        case "paneOwner": self = try .paneOwner(container.decode(PaneOwnerDTO.self, forKey: .value))
        case "vcsStatus": self = try .vcsStatus(container.decode(VCSStatusDTO.self, forKey: .value))
        case "vcsBranches": self = try .vcsBranches(container.decode(VCSBranchesDTO.self, forKey: .value))
        case "vcsPRCreated": self = try .vcsPRCreated(container.decode(VCSCreatePRResultDTO.self, forKey: .value))
        case "projectLogo": self = try .projectLogo(container.decode(ProjectLogoDTO.self, forKey: .value))
        case "notifications": self = try .notifications(container.decode([NotificationDTO].self, forKey: .value))
        case "ok": self = .ok
        default: throw DecodingError.dataCorruptedError(forKey: .type, in: container, debugDescription: "Unknown result type: \(type)")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .projects(v): try container.encode("projects", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .worktrees(v): try container.encode("worktrees", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .workspace(v): try container.encode("workspace", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .tab(v): try container.encode("tab", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .terminalContent(v): try container.encode("terminalContent", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .terminalCells(v): try container.encode("terminalCells", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .deviceInfo(v): try container.encode("deviceInfo", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .pairing(v): try container.encode("pairing", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .paneOwner(v): try container.encode("paneOwner", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .vcsStatus(v): try container.encode("vcsStatus", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .vcsBranches(v): try container.encode("vcsBranches", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .vcsPRCreated(v): try container.encode("vcsPRCreated", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .projectLogo(v): try container.encode("projectLogo", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .notifications(v): try container.encode("notifications", forKey: .type)
            try container.encode(v, forKey: .value)
        case .ok: try container.encode("ok", forKey: .type)
        }
    }
}

public enum MuxyEventKind: String, Codable, Sendable {
    case workspaceChanged
    case terminalOutput
    case terminalSnapshot
    case notificationReceived
    case projectsChanged
    case paneOwnershipChanged
    case themeChanged
}

public enum MuxyEventData: Codable, Sendable {
    case workspace(WorkspaceDTO)
    case terminalOutput(TerminalOutputEventDTO)
    case terminalSnapshot(TerminalOutputEventDTO)
    case notification(NotificationDTO)
    case projects([ProjectDTO])
    case paneOwnership(PaneOwnershipEventDTO)
    case deviceTheme(DeviceThemeEventDTO)

    private enum CodingKeys: String, CodingKey {
        case type
        case value
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)
        switch type {
        case "workspace": self = try .workspace(container.decode(WorkspaceDTO.self, forKey: .value))
        case "terminalOutput": self = try .terminalOutput(container.decode(TerminalOutputEventDTO.self, forKey: .value))
        case "terminalSnapshot": self = try .terminalSnapshot(container.decode(TerminalOutputEventDTO.self, forKey: .value))
        case "notification": self = try .notification(container.decode(NotificationDTO.self, forKey: .value))
        case "projects": self = try .projects(container.decode([ProjectDTO].self, forKey: .value))
        case "paneOwnership": self = try .paneOwnership(container.decode(PaneOwnershipEventDTO.self, forKey: .value))
        case "deviceTheme": self = try .deviceTheme(container.decode(DeviceThemeEventDTO.self, forKey: .value))
        default: throw DecodingError.dataCorruptedError(forKey: .type, in: container, debugDescription: "Unknown event data type: \(type)")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .workspace(v): try container.encode("workspace", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .terminalOutput(v): try container.encode("terminalOutput", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .terminalSnapshot(v): try container.encode("terminalSnapshot", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .notification(v): try container.encode("notification", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .projects(v): try container.encode("projects", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .paneOwnership(v): try container.encode("paneOwnership", forKey: .type)
            try container.encode(v, forKey: .value)
        case let .deviceTheme(v): try container.encode("deviceTheme", forKey: .type)
            try container.encode(v, forKey: .value)
        }
    }
}
