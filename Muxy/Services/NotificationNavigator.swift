import Foundation

struct NavigationContext {
    let projectID: UUID
    let worktreeID: UUID
    let worktreePath: String
    let areaID: UUID
    let tabID: UUID
}

@MainActor
enum NotificationNavigator {
    static func resolveContext(
        for paneID: UUID,
        appState: AppState,
        worktreeStore: WorktreeStore
    ) -> NavigationContext? {
        for (key, root) in appState.workspaceRoots {
            for area in root.allAreas() {
                for tab in area.tabs {
                    guard tab.content.pane?.id == paneID else { continue }
                    let path = worktreeStore.worktree(
                        projectID: key.projectID,
                        worktreeID: key.worktreeID
                    )?.path ?? area.projectPath
                    return NavigationContext(
                        projectID: key.projectID,
                        worktreeID: key.worktreeID,
                        worktreePath: path,
                        areaID: area.id,
                        tabID: tab.id
                    )
                }
            }
        }
        return nil
    }

    static func navigate(
        to notification: MuxyNotification,
        appState: AppState,
        notificationStore: NotificationStore
    ) {
        if appState.activeProjectID != notification.projectID
            || appState.activeWorktreeID[notification.projectID] != notification.worktreeID
        {
            appState.dispatch(.selectProject(
                projectID: notification.projectID,
                worktreeID: notification.worktreeID,
                worktreePath: notification.worktreePath
            ))
        }

        appState.dispatch(.focusArea(
            projectID: notification.projectID,
            areaID: notification.areaID
        ))

        appState.dispatch(.selectTab(
            projectID: notification.projectID,
            areaID: notification.areaID,
            tabID: notification.tabID
        ))

        notificationStore.markAsRead(notification.id)
    }

    static func isFocused(
        _ notification: MuxyNotification,
        appState: AppState
    ) -> Bool {
        guard appState.activeProjectID == notification.projectID else { return false }
        guard appState.activeWorktreeID[notification.projectID] == notification.worktreeID else { return false }
        guard let key = appState.activeWorktreeKey(for: notification.projectID) else { return false }
        guard appState.focusedAreaID[key] == notification.areaID else { return false }
        guard let area = appState.workspaceRoots[key]?.findArea(id: notification.areaID) else { return false }
        return area.activeTabID == notification.tabID
    }
}
