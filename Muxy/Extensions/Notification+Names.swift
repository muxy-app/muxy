import Foundation

extension Notification.Name {
    static let renameActiveTab = Notification.Name("MuxyRenameActiveTab")
    static let toggleThemePicker = Notification.Name("MuxyToggleThemePicker")
    static let themeDidChange = Notification.Name("MuxyThemeDidChange")
    static let findInTerminal = Notification.Name("MuxyFindInTerminal")
    static let toggleFileTree = Notification.Name("MuxyToggleFileTree")
    static let refocusActiveTerminal = Notification.Name("MuxyRefocusActiveTerminal")
    static let quickOpen = Notification.Name("MuxyQuickOpen")
    static let terminalOmnibox = Notification.Name("MuxyTerminalOmnibox")
    static let openProjectPicker = Notification.Name("MuxyOpenProjectPicker")
    static let openSettingsModal = Notification.Name("MuxyOpenSettingsModal")
    static let openExtensionsModal = Notification.Name("MuxyOpenExtensionsModal")
    static let openExtensionInstall = Notification.Name("MuxyOpenExtensionInstall")
    static let openExtensionDirectoryAsProject = Notification.Name("MuxyOpenExtensionDirectoryAsProject")
    static let focusProjectPickerDefaultLocation = Notification.Name("MuxyFocusProjectPickerDefaultLocation")
    static let saveActiveEditor = Notification.Name("MuxySaveActiveEditor")
    static let windowFullScreenDidChange = Notification.Name("MuxyWindowFullScreenDidChange")
    static let toggleSidebar = Notification.Name("MuxyToggleSidebar")
    static let toggleNotificationPanel = Notification.Name("MuxyToggleNotificationPanel")
    static let vcsRepoDidChange = Notification.Name("MuxyVCSRepoDidChange")
    static let vcsDidRefresh = Notification.Name("MuxyVCSDidRefresh")
    static let externalDragHoverChanged = Notification.Name("MuxyExternalDragHoverChanged")
    static let toggleRichInput = Notification.Name("MuxyToggleRichInput")
    static let toggleVoiceRecording = Notification.Name("MuxyToggleVoiceRecording")
}

enum ExternalDragHoverUserInfoKey {
    static let isHovering = "isHovering"
    static let areaID = "areaID"
}

enum OpenExtensionDirectoryUserInfoKey {
    static let path = "path"
}

enum ExtensionInstallUserInfoKey {
    static let name = "name"
}
