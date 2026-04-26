import Foundation

@MainActor
@Observable
final class TerminalTab: Identifiable {
    enum Kind: String, Codable {
        case terminal
        case vcs
        case editor
        case diffViewer
        case claude
        case gemini
    }

    enum Content {
        case terminal(TerminalPaneState)
        case vcs(VCSTabState)
        case editor(EditorTabState)
        case diffViewer(DiffViewerTabState)
        case claude(TerminalPaneState)
        case gemini(TerminalPaneState)

        var kind: Kind {
            switch self {
            case .terminal: .terminal
            case .vcs: .vcs
            case .editor: .editor
            case .diffViewer: .diffViewer
            case .claude: .claude
            case .gemini: .gemini
            }
        }

        var pane: TerminalPaneState? {
            switch self {
            case let .terminal(pane): pane
            case let .claude(pane): pane
            case let .gemini(pane): pane
            default: nil
            }
        }

        var vcsState: VCSTabState? {
            guard case let .vcs(state) = self else { return nil }
            return state
        }

        var editorState: EditorTabState? {
            guard case let .editor(state) = self else { return nil }
            return state
        }

        var diffViewerState: DiffViewerTabState? {
            guard case let .diffViewer(state) = self else { return nil }
            return state
        }

        var projectPath: String {
            switch self {
            case let .terminal(pane): pane.projectPath
            case let .vcs(state): state.projectPath
            case let .editor(state): state.projectPath
            case let .diffViewer(state): state.projectPath
            case let .claude(pane): pane.projectPath
            case let .gemini(pane): pane.projectPath
            }
        }
    }

    let id = UUID()
    var customTitle: String?
    var colorID: String?
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
        case let .editor(state):
            return state.displayTitle
        case let .diffViewer(state):
            return state.displayTitle
        case let .claude(pane):
            return pane.title
        case let .gemini(pane):
            return pane.title
        }
    }

    init(pane: TerminalPaneState, kind: Kind = .terminal) {
        switch kind {
        case .claude: content = .claude(pane)
        case .gemini: content = .gemini(pane)
        default: content = .terminal(pane)
        }
    }

    init(vcsState: VCSTabState) {
        content = .vcs(vcsState)
    }

    init(editorState: EditorTabState) {
        content = .editor(editorState)
    }

    init(diffViewerState: DiffViewerTabState) {
        content = .diffViewer(diffViewerState)
    }

    init(restoring snapshot: TerminalTabSnapshot) {
        customTitle = snapshot.customTitle
        colorID = snapshot.colorID
        isPinned = snapshot.isPinned
        let pane = TerminalPaneState(projectPath: snapshot.projectPath, title: snapshot.paneTitle)
        switch snapshot.kind {
        case .terminal:
            content = .terminal(pane)
        case .vcs:
            content = .vcs(VCSTabState(projectPath: snapshot.projectPath))
        case .editor:
            if let filePath = snapshot.filePath {
                content = .editor(EditorTabState(projectPath: snapshot.projectPath, filePath: filePath))
            } else {
                content = .terminal(pane)
            }
        case .diffViewer:
            content = .terminal(pane)
        case .claude:
            content = .claude(pane)
        case .gemini:
            content = .gemini(pane)
        }
    }

    func snapshot() -> TerminalTabSnapshot {
        TerminalTabSnapshot(
            kind: content.kind,
            customTitle: customTitle,
            colorID: colorID,
            isPinned: isPinned,
            projectPath: content.projectPath,
            paneTitle: content.pane?.title,
            filePath: content.editorState?.filePath
        )
    }
}
