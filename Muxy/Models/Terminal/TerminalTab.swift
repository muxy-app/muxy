import Foundation

@MainActor
@Observable
final class TerminalTab: Identifiable {
    enum Kind: String, Codable {
        case terminal
        case extensionWebView
        case browser
    }

    enum Content {
        case terminal(TerminalPaneState)
        case extensionWebView(ExtensionTabState)
        case browser(BrowserTabState)

        var kind: Kind {
            switch self {
            case .terminal: .terminal
            case .extensionWebView: .extensionWebView
            case .browser: .browser
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

        var browserState: BrowserTabState? {
            guard case let .browser(state) = self else { return nil }
            return state
        }

        var projectPath: String {
            switch self {
            case let .terminal(pane): pane.projectPath
            case let .extensionWebView(state): state.projectPath
            case let .browser(state): state.projectPath
            }
        }
    }

    let id: UUID
    var parentTabID: UUID?
    var customTitle: String?
    var colorID: String?
    var customIcon: String?
    var isPinned: Bool = false
    var internalPanes: InternalPaneNode?
    var focusedPaneID: UUID?
    let content: Content

    var kind: Kind { content.kind }

    var displayPane: TerminalPaneState? {
        guard case let .terminal(pane) = content else { return nil }
        guard let internalPanes,
              let focusedPaneID,
              let focusedPane = internalPanes.findPane(id: focusedPaneID)
        else { return pane }
        return focusedPane
    }

    var terminalPanes: [TerminalPaneState] {
        internalPanes?.allPanes() ?? content.pane.map { [$0] } ?? []
    }

    func containsTerminalPane(id: UUID) -> Bool {
        terminalPanes.contains { $0.id == id }
    }

    var title: String {
        if let customTitle {
            return customTitle
        }
        switch content {
        case .terminal:
            return displayPane?.displayTitle ?? "Terminal"
        case let .extensionWebView(state):
            return state.displayTitle
        case let .browser(state):
            return state.displayTitle
        }
    }

    init(pane: TerminalPaneState, parentTabID: UUID? = nil) {
        id = UUID()
        self.parentTabID = parentTabID
        content = .terminal(pane)
    }

    init(extensionState: ExtensionTabState, parentTabID: UUID? = nil) {
        id = UUID()
        self.parentTabID = parentTabID
        content = .extensionWebView(extensionState)
    }

    init(browserState: BrowserTabState, parentTabID: UUID? = nil) {
        id = UUID()
        self.parentTabID = parentTabID
        content = .browser(browserState)
    }

    init(restoring snapshot: TerminalTabSnapshot) {
        id = snapshot.id
        parentTabID = snapshot.parentTabID
        customTitle = snapshot.customTitle
        colorID = snapshot.colorID
        customIcon = snapshot.customIcon
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
        case .browser:
            let browserState = BrowserTabState(
                projectPath: snapshot.projectPath,
                url: snapshot.browserURL.flatMap(URL.init(string:)),
                profileID: snapshot.browserProfileID.flatMap(UUID.init(uuidString:)) ?? BrowserProfile.defaultID
            )
            browserState.shouldFocusAddressOnOpen = false
            content = .browser(browserState)
        }
        internalPanes = snapshot.internalPanes?.toInternalPaneNode()
        focusedPaneID = snapshot.focusedPaneID.flatMap(UUID.init(uuidString:))
    }

    func snapshot() -> TerminalTabSnapshot {
        TerminalTabSnapshot(
            kind: content.kind,
            id: id,
            parentTabID: parentTabID,
            customTitle: customTitle,
            colorID: colorID,
            customIcon: customIcon,
            isPinned: isPinned,
            projectPath: content.projectPath,
            paneTitle: extensionTabDefaultTitle ?? content.pane?.title,
            paneID: content.pane?.id,
            currentWorkingDirectory: content.pane?.currentWorkingDirectory,
            extensionID: content.extensionState?.extensionID,
            extensionTabTypeID: content.extensionState?.tabTypeID,
            extensionTabData: content.extensionState?.data,
            browserURL: content.browserState?.url?.absoluteString,
            browserProfileID: content.browserState?.profileID.uuidString,
            internalPanes: internalPanes.map(InternalPaneNodeSnapshot.init(node:)),
            focusedPaneID: focusedPaneID?.uuidString
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
