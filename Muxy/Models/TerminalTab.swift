import Foundation

@MainActor
@Observable
final class TerminalTab: Identifiable {
    enum Kind: String, Codable {
        case terminal
        case vcs
        case editor
        case diffViewer
        case imageViewer
    }

    enum Content {
        case terminal(TerminalPaneState)
        case vcs(VCSTabState)
        case editor(EditorTabState)
        case diffViewer(DiffViewerTabState)
        case imageViewer(ImageViewerTabState)

        var kind: Kind {
            switch self {
            case .terminal: .terminal
            case .vcs: .vcs
            case .editor: .editor
            case .diffViewer: .diffViewer
            case .imageViewer: .imageViewer
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

        var editorState: EditorTabState? {
            guard case let .editor(state) = self else { return nil }
            return state
        }

        var diffViewerState: DiffViewerTabState? {
            guard case let .diffViewer(state) = self else { return nil }
            return state
        }

        var imageViewerState: ImageViewerTabState? {
            guard case let .imageViewer(state) = self else { return nil }
            return state
        }

        var projectPath: String {
            switch self {
            case let .terminal(pane): pane.projectPath
            case let .vcs(state): state.projectPath
            case let .editor(state): state.projectPath
            case let .diffViewer(state): state.projectPath
            case let .imageViewer(state): state.projectPath
            }
        }
    }

    let id: UUID
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
        case let .imageViewer(state):
            return state.displayTitle
        }
    }

    init(pane: TerminalPaneState) {
        id = UUID()
        content = .terminal(pane)
    }

    init(vcsState: VCSTabState) {
        id = UUID()
        content = .vcs(vcsState)
    }

    init(editorState: EditorTabState) {
        id = UUID()
        content = .editor(editorState)
    }

    init(diffViewerState: DiffViewerTabState) {
        id = UUID()
        content = .diffViewer(diffViewerState)
    }

    init(imageViewerState: ImageViewerTabState) {
        id = UUID()
        content = .imageViewer(imageViewerState)
    }

    init(restoring snapshot: TerminalTabSnapshot, restoredSession: TerminalSessionSnapshot? = nil) {
        id = snapshot.id
        customTitle = snapshot.customTitle
        colorID = snapshot.colorID
        isPinned = snapshot.isPinned
        switch snapshot.kind {
        case .terminal:
            content = .terminal(TerminalPaneState(
                id: snapshot.paneID ?? UUID(),
                projectPath: snapshot.projectPath,
                title: snapshot.paneTitle,
                initialWorkingDirectory: restoredSession?.workingDirectory ?? snapshot.currentWorkingDirectory,
                restoredSession: restoredSession
            ))
        case .vcs:
            content = .vcs(VCSStateStore.shared.state(for: snapshot.projectPath))
        case .editor:
            if let filePath = snapshot.filePath {
                content = .editor(EditorTabState(projectPath: snapshot.projectPath, filePath: filePath))
            } else {
                content = .terminal(TerminalPaneState(projectPath: snapshot.projectPath, title: snapshot.paneTitle))
            }
        case .diffViewer:
            content = .terminal(TerminalPaneState(projectPath: snapshot.projectPath, title: snapshot.paneTitle))
        case .imageViewer:
            if let filePath = snapshot.filePath {
                content = .imageViewer(ImageViewerTabState(projectPath: snapshot.projectPath, filePath: filePath))
            } else {
                content = .terminal(TerminalPaneState(projectPath: snapshot.projectPath, title: snapshot.paneTitle))
            }
        }
    }

    func snapshot() -> TerminalTabSnapshot {
        TerminalTabSnapshot(
            kind: content.kind,
            id: id,
            customTitle: customTitle,
            colorID: colorID,
            isPinned: isPinned,
            projectPath: content.projectPath,
            paneTitle: content.pane?.title,
            paneID: content.pane?.id,
            filePath: content.editorState?.filePath ?? content.imageViewerState?.filePath,
            currentWorkingDirectory: content.pane?.currentWorkingDirectory
        )
    }
}
