import Foundation

@MainActor
enum ExtensionEventEmitter {
    struct WorkspaceSnapshot {
        let activeProjectID: UUID?
        let activeWorktreeID: [UUID: UUID]
        let panes: Set<UUID>
        let tabs: Set<UUID>
        let focusedAreaID: [WorktreeKey: UUID]
        let activeTabIDPerArea: [UUID: UUID]
    }

    static func snapshot(from appState: AppState) -> WorkspaceSnapshot {
        var panes = Set<UUID>()
        var tabs = Set<UUID>()
        var activeTabs: [UUID: UUID] = [:]
        for root in appState.workspaceRoots.values {
            for area in root.allAreas() {
                if let activeTabID = area.activeTabID {
                    activeTabs[area.id] = activeTabID
                }
                for tab in area.tabs {
                    tabs.insert(tab.id)
                    if let pane = tab.content.pane {
                        panes.insert(pane.id)
                    }
                }
            }
        }
        return WorkspaceSnapshot(
            activeProjectID: appState.activeProjectID,
            activeWorktreeID: appState.activeWorktreeID,
            panes: panes,
            tabs: tabs,
            focusedAreaID: appState.focusedAreaID,
            activeTabIDPerArea: activeTabs
        )
    }

    static func emit(before: WorkspaceSnapshot, after: WorkspaceSnapshot) {
        let server = NotificationSocketServer.shared

        for paneID in after.panes.subtracting(before.panes) {
            server.broadcast(event: ExtensionEvent(
                name: ExtensionEventName.paneCreated,
                payload: ["paneID": paneID.uuidString]
            ))
        }
        for paneID in before.panes.subtracting(after.panes) {
            server.broadcast(event: ExtensionEvent(
                name: ExtensionEventName.paneClosed,
                payload: ["paneID": paneID.uuidString]
            ))
        }
        for tabID in after.tabs.subtracting(before.tabs) {
            server.broadcast(event: ExtensionEvent(
                name: ExtensionEventName.tabCreated,
                payload: ["tabID": tabID.uuidString]
            ))
        }

        if before.activeProjectID != after.activeProjectID, let projectID = after.activeProjectID {
            server.broadcast(event: ExtensionEvent(
                name: ExtensionEventName.projectSwitched,
                payload: ["projectID": projectID.uuidString]
            ))
        }

        for (projectID, worktreeID) in after.activeWorktreeID where before.activeWorktreeID[projectID] != worktreeID {
            server.broadcast(event: ExtensionEvent(
                name: ExtensionEventName.worktreeSwitched,
                payload: [
                    "projectID": projectID.uuidString,
                    "worktreeID": worktreeID.uuidString,
                ]
            ))
        }

        for (areaID, tabID) in after.activeTabIDPerArea where before.activeTabIDPerArea[areaID] != tabID {
            server.broadcast(event: ExtensionEvent(
                name: ExtensionEventName.tabFocused,
                payload: [
                    "areaID": areaID.uuidString,
                    "tabID": tabID.uuidString,
                ]
            ))
        }

        if before.focusedAreaID != after.focusedAreaID {
            for (key, areaID) in after.focusedAreaID where before.focusedAreaID[key] != areaID {
                guard let activeTabID = after.activeTabIDPerArea[areaID] else { continue }
                server.broadcast(event: ExtensionEvent(
                    name: ExtensionEventName.paneFocused,
                    payload: [
                        "projectID": key.projectID.uuidString,
                        "worktreeID": key.worktreeID.uuidString,
                        "areaID": areaID.uuidString,
                        "tabID": activeTabID.uuidString,
                    ]
                ))
            }
        }
    }
}
