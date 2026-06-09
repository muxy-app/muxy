import Foundation

@MainActor
@Observable
final class TerminalTab: Identifiable {
    enum Kind: String, Codable {
        case terminal
        case extensionWebView
    }

    enum Content {
        case terminal(TerminalPaneState)
        case extensionWebView(ExtensionTabState)

        var kind: Kind {
            switch self {
            case .terminal: .terminal
            case .extensionWebView: .extensionWebView
            }
        }

        var pane: TerminalPaneState? {
            guard case let .terminal(pane) = self else { return nil }
            return pane
        }

        var extensionState: ExtensionTabState? {
            guard case let .extensionWebView(state) = self else { return nil }
            return state
        }

        var projectPath: String {
            switch self {
            case let .terminal(pane): pane.projectPath
            case let .extensionWebView(state): state.projectPath
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
        case let .extensionWebView(state):
            return state.displayTitle
        }
    }

    init(pane: TerminalPaneState) {
        id = UUID()
        content = .terminal(pane)
    }

    init(extensionState: ExtensionTabState) {
        id = UUID()
        content = .extensionWebView(extensionState)
    }

    init(restoring snapshot: TerminalTabSnapshot) {
        id = snapshot.id
        customTitle = snapshot.customTitle
        colorID = snapshot.colorID
        isPinned = snapshot.isPinned
        switch snapshot.kind {
        case .terminal:
            let restoredWorkingDirectory = Self.restoredWorkingDirectory(
                snapshot.currentWorkingDirectory,
                projectPath: snapshot.projectPath
            )
            content = .terminal(TerminalPaneState(
                id: snapshot.paneID ?? UUID(),
                projectPath: snapshot.projectPath,
                title: snapshot.paneTitle,
                initialWorkingDirectory: restoredWorkingDirectory
            ))
        case .extensionWebView:
            if let extensionID = snapshot.extensionID,
               let tabTypeID = snapshot.extensionTabTypeID
            {
                content = .extensionWebView(ExtensionTabState(
                    extensionID: extensionID,
                    tabTypeID: tabTypeID,
                    projectPath: snapshot.projectPath,
                    defaultTitle: snapshot.paneTitle,
                    data: snapshot.extensionTabData
                ))
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
            paneTitle: extensionTabDefaultTitle ?? content.pane?.title,
            paneID: content.pane?.id,
            currentWorkingDirectory: content.pane?.currentWorkingDirectory,
            extensionID: content.extensionState?.extensionID,
            extensionTabTypeID: content.extensionState?.tabTypeID,
            extensionTabData: content.extensionState?.data
        )
    }

    private var extensionTabDefaultTitle: String? {
        content.extensionState?.defaultTitle
    }

    private static func restoredWorkingDirectory(_ path: String?, projectPath: String) -> String? {
        guard let path else { return nil }
        let standardizedPath = URL(fileURLWithPath: path).standardizedFileURL.path
        let standardizedProjectPath = URL(fileURLWithPath: projectPath).standardizedFileURL.path
        guard standardizedPath == standardizedProjectPath || standardizedPath.hasPrefix(standardizedProjectPath + "/") else {
            return nil
        }
        return path
    }
}
