import Foundation

extension Notification.Name {
    static let renameActiveTab = Notification.Name("MuxyRenameActiveTab")
    static let toggleThemePicker = Notification.Name("MuxyToggleThemePicker")
    static let themeDidChange = Notification.Name("MuxyThemeDidChange")
    static let findInTerminal = Notification.Name("MuxyFindInTerminal")
    static let openVCSWindow = Notification.Name("MuxyOpenVCSWindow")
    static let toggleAttachedVCS = Notification.Name("MuxyToggleAttachedVCS")
    static let toggleFileTree = Notification.Name("MuxyToggleFileTree")
    static let quickOpen = Notification.Name("MuxyQuickOpen")
    static let switchWorktree = Notification.Name("MuxySwitchWorktree")
    static let saveActiveEditor = Notification.Name("MuxySaveActiveEditor")
    static let windowFullScreenDidChange = Notification.Name("MuxyWindowFullScreenDidChange")
    static let toggleSidebar = Notification.Name("MuxyToggleSidebar")
    static let toggleNotificationPanel = Notification.Name("MuxyToggleNotificationPanel")
    static let vcsRepoDidChange = Notification.Name("MuxyVCSRepoDidChange")
}
