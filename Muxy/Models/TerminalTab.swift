import Foundation

@MainActor
@Observable
final class TerminalTab: Identifiable {
    enum Kind: String, Codable {
        case terminal
        case vcs
    }

    enum Content {
        case terminal(TerminalPaneState)
        case vcs(VCSTabState)

        var kind: Kind {
            switch self {
            case .terminal: .terminal
            case .vcs: .vcs
            }
        }

        var pane: TerminalPaneState? {
            guard case let .terminal(pane) = self else { return nil }
            return pane
        }

        var vcsState: VCSTabState? {
            guard case let .vcs(state) = self else { return nil }
            return state
        }

        var projectPath: String {
            switch self {
            case let .terminal(pane): pane.projectPath
            case let .vcs(state): state.projectPath
            }
        }
    }

    let id = UUID()
    var customTitle: String?
    var isPinned: Bool = false
    let content: Content

    var kind: Kind { content.kind }

    var title: String {
        if let customTitle {
            return customTitle
        }
        switch content {
        case let .terminal(pane):
            return pane.title
        case .vcs:
            return "Git Diff"
        }
    }

    init(pane: TerminalPaneState) {
        content = .terminal(pane)
    }

    init(vcsState: VCSTabState) {
        content = .vcs(vcsState)
    }

    init(restoring snapshot: TerminalTabSnapshot) {
        customTitle = snapshot.customTitle
        isPinned = snapshot.isPinned
        switch snapshot.kind {
        case .terminal:
            content = .terminal(TerminalPaneState(projectPath: snapshot.projectPath, title: snapshot.paneTitle))
        case .vcs:
            content = .vcs(VCSTabState(projectPath: snapshot.projectPath))
        }
    }

    func snapshot() -> TerminalTabSnapshot {
        TerminalTabSnapshot(
            kind: content.kind,
            customTitle: customTitle,
            isPinned: isPinned,
            projectPath: content.projectPath,
            paneTitle: content.pane?.title
        )
    }
}
