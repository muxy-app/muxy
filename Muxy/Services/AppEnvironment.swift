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
    let groupPersistence: any ProjectGroupPersisting

    static let live = Self(
        selectionStore: UserDefaultsActiveProjectSelectionStore(),
        terminalViews: TerminalViewRegistry.shared,
        projectPersistence: FileProjectPersistence(),
        workspacePersistence: FileWorkspacePersistence(),
        worktreePersistence: FileWorktreePersistence(),
        groupPersistence: FileProjectGroupPersistence()
    )
}
