import Foundation

@MainActor
struct AppEnvironment {
    static let isDevelopment: Bool = {
        #if DEBUG
        true
        #else
        false
        #endif
    }()

    let selectionStore: any ActiveProjectSelectionStoring
    let terminalViews: any TerminalViewRemoving
    let projectPersistence: any ProjectPersisting
    let workspacePersistence: any WorkspacePersisting
    let worktreePersistence: any WorktreePersisting
    let projectGroupPersistence: any ProjectGroupPersisting
    let remoteDevicePersistence: any RemoteDevicePersisting
    let browserProfilePersistence: any BrowserProfilePersisting
    let browserHistoryPersistence: any BrowserHistoryPersisting

    static let live = Self(
        selectionStore: UserDefaultsActiveProjectSelectionStore(),
        terminalViews: TerminalViewRegistry.shared,
        projectPersistence: FileProjectPersistence(),
        workspacePersistence: FileWorkspacePersistence(),
        worktreePersistence: FileWorktreePersistence(),
        projectGroupPersistence: FileProjectGroupPersistence(),
        remoteDevicePersistence: FileRemoteDevicePersistence(),
        browserProfilePersistence: FileBrowserProfilePersistence(),
        browserHistoryPersistence: FileBrowserHistoryPersistence()
    )
}
