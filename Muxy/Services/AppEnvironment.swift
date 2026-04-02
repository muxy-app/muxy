import Foundation

@MainActor
struct AppEnvironment {
    let selectionStore: any ActiveProjectSelectionStoring
    let terminalViews: any TerminalViewRemoving
    let projectPersistence: any ProjectPersisting

    static let live = Self(
        selectionStore: UserDefaultsActiveProjectSelectionStore(),
        terminalViews: TerminalViewRegistry.shared,
        projectPersistence: FileProjectPersistence()
    )
}
