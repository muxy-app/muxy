import Foundation

extension Notification.Name {
    static let renameActiveTab = Notification.Name("MuxyRenameActiveTab")
    static let toggleThemePicker = Notification.Name("MuxyToggleThemePicker")
    static let themeDidChange = Notification.Name("MuxyThemeDidChange")
    static let localizationDidChange = Notification.Name("MuxyLocalizationDidChange")
    static let ghosttyConfigurationDidChange = Notification.Name("MuxyGhosttyConfigurationDidChange")
    static let findInTerminal = Notification.Name("MuxyFindInTerminal")
    static let refocusActiveTerminal = Notification.Name("MuxyRefocusActiveTerminal")
    static let terminalOmnibox = Notification.Name("MuxyTerminalOmnibox")
    static let openProjectPicker = Notification.Name("MuxyOpenProjectPicker")
    static let openRemoteProjectPicker = Notification.Name("MuxyOpenRemoteProjectPicker")
    static let openSettingsModal = Notification.Name("MuxyOpenSettingsModal")
    static let openExtensionsModal = Notification.Name("MuxyOpenExtensionsModal")
    static let openWhatsNewModal = Notification.Name("MuxyOpenWhatsNewModal")
    static let openExtensionInstall = Notification.Name("MuxyOpenExtensionInstall")
    static let openExtensionBrowse = Notification.Name("MuxyOpenExtensionBrowse")
    static let openExtensionDirectoryAsProject = Notification.Name("MuxyOpenExtensionDirectoryAsProject")
    static let focusProjectPickerDefaultLocation = Notification.Name("MuxyFocusProjectPickerDefaultLocation")
    static let focusRemoteDevicesSettings = Notification.Name("MuxyFocusRemoteDevicesSettings")
    static let focusBrowserSettings = Notification.Name("MuxyFocusBrowserSettings")
    static let focusTerminalSettings = Notification.Name("MuxyFocusTerminalSettings")
    static let focusQuickTerminalShortcut = Notification.Name("MuxyFocusQuickTerminalShortcut")
    static let quickTerminalEnabledDidChange = Notification.Name("MuxyQuickTerminalEnabledDidChange")
    static let windowFullScreenDidChange = Notification.Name("MuxyWindowFullScreenDidChange")
    static let toggleSidebar = Notification.Name("MuxyToggleSidebar")
    static let toggleAppLayout = Notification.Name("MuxyToggleAppLayout")
    static let toggleNotificationPanel = Notification.Name("MuxyToggleNotificationPanel")
    static let createWorktreeRequested = Notification.Name("MuxyCreateWorktreeRequested")
    static let removeCurrentWorktreeRequested = Notification.Name("MuxyRemoveCurrentWorktreeRequested")
    static let workspaceFilesDidChange = Notification.Name("MuxyWorkspaceFilesDidChange")
    static let vcsRepoDidChange = Notification.Name("MuxyVCSRepoDidChange")
    static let vcsDidRefresh = Notification.Name("MuxyVCSDidRefresh")
    static let externalDragHoverChanged = Notification.Name("MuxyExternalDragHoverChanged")
    static let toggleRichInput = Notification.Name("MuxyToggleRichInput")
    static let toggleComposerVoice = Notification.Name("MuxyToggleComposerVoice")
    static let toggleVoiceRecording = Notification.Name("MuxyToggleVoiceRecording")
    static let toggleExtensionConsole = Notification.Name("MuxyToggleExtensionConsole")
}

struct WorkspaceFilesChange: Sendable {
    private static let projectIDKey = "projectID"
    private static let worktreeIDKey = "worktreeID"
    private static let rootKey = "root"
    private static let pathsKey = "paths"

    let projectID: UUID
    let worktreeID: UUID?
    let root: String
    let paths: [String]

    init(projectID: UUID, worktreeID: UUID?, root: String, paths: [String]) {
        self.projectID = projectID
        self.worktreeID = worktreeID
        self.root = root
        self.paths = paths
    }

    init?(_ notification: Notification) {
        guard let userInfo = notification.userInfo,
              let projectID = userInfo[Self.projectIDKey] as? UUID,
              let root = userInfo[Self.rootKey] as? String,
              let paths = userInfo[Self.pathsKey] as? [String]
        else { return nil }
        self.init(
            projectID: projectID,
            worktreeID: userInfo[Self.worktreeIDKey] as? UUID,
            root: root,
            paths: paths
        )
    }

    func post() {
        var userInfo: [String: Any] = [
            Self.projectIDKey: projectID,
            Self.rootKey: root,
            Self.pathsKey: paths,
        ]
        userInfo[Self.worktreeIDKey] = worktreeID
        NotificationCenter.default.post(name: .workspaceFilesDidChange, object: nil, userInfo: userInfo)
    }
}

enum ExternalDragHoverUserInfoKey {
    static let isHovering = "isHovering"
    static let areaID = "areaID"
}

enum OpenExtensionDirectoryUserInfoKey {
    static let path = "path"
}

enum OpenRemoteProjectPickerUserInfoKey {
    static let deviceID = "deviceID"
}

enum ExtensionInstallUserInfoKey {
    static let name = "name"
}

struct ExtensionsPresentationRequest: Equatable, Identifiable, Sendable {
    private static let idKey = "id"
    private static let categoryKey = "category"

    let id: UUID
    let browseCategory: String?
    var requiresSettingsHandoff: Bool { browseCategory != nil }

    init(id: UUID = UUID(), browseCategory: String? = nil) {
        self.id = id
        self.browseCategory = browseCategory
    }

    init(_ notification: Notification) {
        id = notification.userInfo?[Self.idKey] as? UUID ?? UUID()
        browseCategory = notification.userInfo?[Self.categoryKey] as? String
    }

    func post(notificationCenter: NotificationCenter = .default) {
        var userInfo: [String: Any] = [Self.idKey: id]
        if let browseCategory {
            userInfo[Self.categoryKey] = browseCategory
        }
        notificationCenter.post(name: .openExtensionsModal, object: nil, userInfo: userInfo)
    }

    func postBrowse(notificationCenter: NotificationCenter = .default) {
        guard let browseCategory else { return }
        notificationCenter.post(
            name: .openExtensionBrowse,
            object: nil,
            userInfo: [
                Self.idKey: id,
                Self.categoryKey: browseCategory,
            ]
        )
    }
}
