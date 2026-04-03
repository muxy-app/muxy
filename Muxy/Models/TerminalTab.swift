import Foundation

@MainActor
@Observable
final class TerminalTab: Identifiable {
    enum Kind: String, Codable {
        case terminal
        case vcs
    }

    let id = UUID()
    var customTitle: String?
    var isPinned: Bool = false
    let kind: Kind
    let pane: TerminalPaneState?
    let vcsState: VCSTabState?

    var title: String {
        if let customTitle {
            return customTitle
        }
        switch kind {
        case .terminal:
            return pane?.title ?? "Terminal"
        case .vcs:
            return "Git Diff"
        }
    }

    init(pane: TerminalPaneState) {
        kind = .terminal
        self.pane = pane
        vcsState = nil
    }

    init(vcsState: VCSTabState) {
        kind = .vcs
        pane = nil
        self.vcsState = vcsState
    }

    init(restoring snapshot: TerminalTabSnapshot) {
        customTitle = snapshot.customTitle
        isPinned = snapshot.isPinned
        kind = snapshot.kind
        switch snapshot.kind {
        case .terminal:
            pane = TerminalPaneState(projectPath: snapshot.projectPath, title: snapshot.paneTitle)
            vcsState = nil
        case .vcs:
            pane = nil
            vcsState = VCSTabState(projectPath: snapshot.projectPath)
        }
    }

    func snapshot() -> TerminalTabSnapshot {
        let projectPath: String = switch kind {
        case .terminal:
            pane?.projectPath ?? ""
        case .vcs:
            vcsState?.projectPath ?? ""
        }

        return TerminalTabSnapshot(
            kind: kind,
            customTitle: customTitle,
            isPinned: isPinned,
            projectPath: projectPath,
            paneTitle: pane?.title
        )
    }
}
