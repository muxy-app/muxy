import Foundation

@MainActor
enum ExtensionEventEmitter {
    struct TabContext: Equatable {
        let tabID: UUID
        let paneID: UUID?
        let kind: String
        let projectID: UUID
        let worktreeID: UUID
        let areaID: UUID
        let title: String
        let projectPath: String
        let cwd: String?
        let extensionID: String?
        let tabTypeID: String?
        let data: String?

        var changesRelevantToRestore: [String] {
            [title, projectPath, cwd ?? "", data ?? ""]
        }
    }

    struct WorkspaceSnapshot {
        let activeProjectID: UUID?
        let activeWorktreeID: [UUID: UUID]
        let panes: Set<UUID>
        let tabs: Set<UUID>
        let focusedAreaID: [WorktreeKey: UUID]
        let activeTabIDPerArea: [UUID: UUID]
        let tabContext: [UUID: TabContext]
        let paneContext: [UUID: TabContext]
    }

    static func snapshot(from appState: AppState) -> WorkspaceSnapshot {
        var panes = Set<UUID>()
        var tabs = Set<UUID>()
        var activeTabs: [UUID: UUID] = [:]
        var tabContext: [UUID: TabContext] = [:]
        var paneContext: [UUID: TabContext] = [:]
        for (key, root) in appState.workspaceRoots {
            for area in root.allAreas() {
                if let activeTabID = area.activeTabID {
                    activeTabs[area.id] = activeTabID
                }
                for tab in area.tabs {
                    tabs.insert(tab.id)
                    let context = context(for: tab, areaID: area.id, key: key)
                    tabContext[tab.id] = context
                    if let pane = tab.content.pane {
                        panes.insert(pane.id)
                        paneContext[pane.id] = context
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
            activeTabIDPerArea: activeTabs,
            tabContext: tabContext,
            paneContext: paneContext
        )
    }

    static func emitTabUpdated(forPane paneID: UUID, appState: AppState) {
        guard let located = appState.locateTab(forPane: paneID) else { return }
        let context = context(for: located.tab, areaID: located.areaID, key: located.worktreeKey)
        NotificationSocketServer.shared.broadcast(event: ExtensionEvent(
            name: ExtensionEventName.tabUpdated,
            payload: payload(from: context)
        ))
    }

    private static func context(for tab: TerminalTab, areaID: UUID, key: WorktreeKey) -> TabContext {
        TabContext(
            tabID: tab.id,
            paneID: tab.content.pane?.id,
            kind: tab.kind.rawValue,
            projectID: key.projectID,
            worktreeID: key.worktreeID,
            areaID: areaID,
            title: tab.title,
            projectPath: tab.content.projectPath,
            cwd: tab.content.pane?.currentWorkingDirectory,
            extensionID: tab.content.extensionState?.extensionID,
            tabTypeID: tab.content.extensionState?.tabTypeID,
            data: encodedData(tab.content.extensionState?.data)
        )
    }

    private static func encodedData(_ data: ExtensionJSON?) -> String? {
        guard let data else { return nil }
        guard let encoded = try? JSONEncoder().encode(data) else { return nil }
        return String(data: encoded, encoding: .utf8)
    }

    private static func payload(from context: TabContext) -> [String: String] {
        var payload: [String: String] = [
            "tabID": context.tabID.uuidString,
            "kind": context.kind,
            "projectID": context.projectID.uuidString,
            "worktreeID": context.worktreeID.uuidString,
            "areaID": context.areaID.uuidString,
            "title": context.title,
            "projectPath": context.projectPath,
        ]
        if let paneID = context.paneID {
            payload["paneID"] = paneID.uuidString
        }
        if let cwd = context.cwd {
            payload["cwd"] = cwd
        }
        if let extensionID = context.extensionID {
            payload["extensionID"] = extensionID
        }
        if let tabTypeID = context.tabTypeID {
            payload["tabTypeID"] = tabTypeID
        }
        if let data = context.data {
            payload["data"] = data
        }
        return payload
    }

    private static func tabEventPayload(tabID: UUID, context: TabContext?) -> [String: String] {
        guard let context else { return ["tabID": tabID.uuidString] }
        return payload(from: context)
    }

    private static func paneEventPayload(paneID: UUID, context: TabContext?) -> [String: String] {
        guard let context else { return ["paneID": paneID.uuidString] }
        return payload(from: context)
    }

    static func emit(before: WorkspaceSnapshot, after: WorkspaceSnapshot) {
        let server = NotificationSocketServer.shared

        for paneID in after.panes.subtracting(before.panes) {
            server.broadcast(event: ExtensionEvent(
                name: ExtensionEventName.paneCreated,
                payload: paneEventPayload(paneID: paneID, context: after.paneContext[paneID])
            ))
        }
        for paneID in before.panes.subtracting(after.panes) {
            AgentStatusStore.shared.removePane(paneID)
            server.broadcast(event: ExtensionEvent(
                name: ExtensionEventName.paneClosed,
                payload: paneEventPayload(paneID: paneID, context: before.paneContext[paneID])
            ))
        }
        for tabID in after.tabs.subtracting(before.tabs) {
            server.broadcast(event: ExtensionEvent(
                name: ExtensionEventName.tabCreated,
                payload: tabEventPayload(tabID: tabID, context: after.tabContext[tabID])
            ))
        }
        for tabID in before.tabs.subtracting(after.tabs) {
            server.broadcast(event: ExtensionEvent(
                name: ExtensionEventName.tabClosed,
                payload: tabEventPayload(tabID: tabID, context: before.tabContext[tabID])
            ))
        }
        for tabID in after.tabs.intersection(before.tabs) {
            guard let afterContext = after.tabContext[tabID],
                  let beforeContext = before.tabContext[tabID],
                  afterContext.changesRelevantToRestore != beforeContext.changesRelevantToRestore
            else { continue }
            server.broadcast(event: ExtensionEvent(
                name: ExtensionEventName.tabUpdated,
                payload: payload(from: afterContext)
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
