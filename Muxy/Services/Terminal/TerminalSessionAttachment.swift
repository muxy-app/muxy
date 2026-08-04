import Foundation

@MainActor
enum TerminalSessionAttachment {
    enum Outcome: Equatable {
        case attached
        case alreadyAttached
        case unavailable
    }

    static func attachToActivePane(sessionID: UUID, appState: AppState) -> Outcome {
        guard let paneID = NotificationNavigator.activePaneID(appState: appState),
              let location = appState.locatePane(paneID: paneID)
        else { return .unavailable }
        releaseOwners(of: sessionID, excluding: paneID, in: appState.allTerminalPanes)
        return attach(sessionID: sessionID, to: location.pane)
    }

    static func attach(sessionID: UUID, to pane: TerminalPaneState) -> Outcome {
        guard pane.sessionID != sessionID else { return .alreadyAttached }
        TerminalViewRegistry.shared.releaseView(for: pane.id)
        pane.sessionID = sessionID
        return .attached
    }

    @discardableResult
    static func releaseOwners(
        of sessionID: UUID,
        excluding paneID: UUID,
        in panes: [TerminalPaneState]
    ) -> [TerminalPaneState] {
        let owners = panes.filter { $0.id != paneID && $0.sessionID == sessionID }
        for owner in owners {
            TerminalViewRegistry.shared.releaseView(for: owner.id)
            owner.sessionID = owner.id
        }
        return owners
    }

    @discardableResult
    static func attachInNewTab(
        sessionID: UUID,
        title: String?,
        key: WorktreeKey,
        appState: AppState
    ) -> Outcome {
        guard let existing = appState.allTerminalPanes.first(where: { $0.sessionID == sessionID }) else {
            appState.dispatch(.createSessionTab(key: key, areaID: nil, sessionID: sessionID, title: title))
            return .attached
        }
        guard let location = appState.locateTab(forPane: existing.id) else { return .alreadyAttached }
        appState.dispatch(.selectTabInWorktree(
            key: location.worktreeKey,
            areaID: location.areaID,
            tabID: location.tab.id
        ))
        return .alreadyAttached
    }
}
