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

    static func activeTabID(appState: AppState) -> UUID? {
        guard let projectID = appState.activeProjectID,
              let key = appState.activeWorktreeKey(for: projectID),
              let areaID = appState.focusedAreaID[key],
              let area = appState.workspaceRoots[key]?.findArea(id: areaID)
        else { return nil }
        return area.activeTabID
    }

    static func activePaneID(appState: AppState) -> UUID? {
        guard let projectID = appState.activeProjectID,
              let key = appState.activeWorktreeKey(for: projectID),
              let areaID = appState.focusedAreaID[key],
              let area = appState.workspaceRoots[key]?.findArea(id: areaID),
              let activeTabID = area.activeTabID,
              let tab = area.tabs.first(where: { $0.id == activeTabID })
        else { return nil }
        return tab.content.pane?.id
    }

    static func isActiveTab(_ tabID: UUID, appState: AppState) -> Bool {
        activeTabID(appState: appState) == tabID
    }
}
