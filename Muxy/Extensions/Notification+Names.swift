import Foundation

extension Notification.Name {
    static let renameActiveTab = Notification.Name("MuxyRenameActiveTab")
    static let toggleThemePicker = Notification.Name("MuxyToggleThemePicker")
    static let themeDidChange = Notification.Name("MuxyThemeDidChange")
    static let findInTerminal = Notification.Name("MuxyFindInTerminal")
    static let refocusActiveTerminal = Notification.Name("MuxyRefocusActiveTerminal")
    static let terminalOmnibox = Notification.Name("MuxyTerminalOmnibox")
    static let openProjectPicker = Notification.Name("MuxyOpenProjectPicker")
    static let openRemoteProjectPicker = Notification.Name("MuxyOpenRemoteProjectPicker")
    static let openSettingsModal = Notification.Name("MuxyOpenSettingsModal")
    static let openExtensionsModal = Notification.Name("MuxyOpenExtensionsModal")
    static let openWhatsNewModal = Notification.Name("MuxyOpenWhatsNewModal")
    static let openExtensionInstall = Notification.Name("MuxyOpenExtensionInstall")
    static let openExtensionDirectoryAsProject = Notification.Name("MuxyOpenExtensionDirectoryAsProject")
    static let focusProjectPickerDefaultLocation = Notification.Name("MuxyFocusProjectPickerDefaultLocation")
    static let focusRemoteDevicesSettings = Notification.Name("MuxyFocusRemoteDevicesSettings")
    static let focusBrowserSettings = Notification.Name("MuxyFocusBrowserSettings")
    static let windowFullScreenDidChange = Notification.Name("MuxyWindowFullScreenDidChange")
    static let toggleSidebar = Notification.Name("MuxyToggleSidebar")
    static let toggleAppLayout = Notification.Name("MuxyToggleAppLayout")
    static let toggleNotificationPanel = Notification.Name("MuxyToggleNotificationPanel")
    static let createWorktreeRequested = Notification.Name("MuxyCreateWorktreeRequested")
    static let removeCurrentWorktreeRequested = Notification.Name("MuxyRemoveCurrentWorktreeRequested")
    static let vcsRepoDidChange = Notification.Name("MuxyVCSRepoDidChange")
    static let vcsDidRefresh = Notification.Name("MuxyVCSDidRefresh")
    static let externalDragHoverChanged = Notification.Name("MuxyExternalDragHoverChanged")
    static let toggleRichInput = Notification.Name("MuxyToggleRichInput")
    static let toggleVoiceRecording = Notification.Name("MuxyToggleVoiceRecording")
    static let toggleExtensionConsole = Notification.Name("MuxyToggleExtensionConsole")
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
