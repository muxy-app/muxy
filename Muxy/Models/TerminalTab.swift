import Foundation

@MainActor
@Observable
final class TerminalTab: Identifiable {
    let id = UUID()
    var customTitle: String?
    var isPinned: Bool = false
    let pane: TerminalPaneState

    var title: String { customTitle ?? pane.title }

    init(pane: TerminalPaneState) {
        self.pane = pane
    }

    init(restoring snapshot: TerminalTabSnapshot) {
        customTitle = snapshot.customTitle
        isPinned = snapshot.isPinned
        pane = TerminalPaneState(projectPath: snapshot.projectPath, title: snapshot.paneTitle)
    }

    func snapshot() -> TerminalTabSnapshot {
        TerminalTabSnapshot(
            customTitle: customTitle,
            isPinned: isPinned,
            projectPath: pane.projectPath,
            paneTitle: pane.title
        )
    }
}
