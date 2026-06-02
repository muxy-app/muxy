import Foundation

@MainActor
enum MuxyAPIDispatcher {
    struct Context {
        let extensionID: String
        let appState: AppState
        let projectStore: ProjectStore?
        let worktreeStore: WorktreeStore?
    }

    static func dispatch(verb: String, args: [String: Any], context: Context) async throws -> Any {
        if let required = MuxyAPI.Permissions.required(for: verb),
           !ExtensionStore.shared.extensionHasPermission(id: context.extensionID, permission: required)
        {
            throw APIError.underlying("permission denied (\(required.rawValue))")
        }
        switch verb {
        case "toast",
             "notifications.notify":
            return try await handleNotify(args: args, context: context)
        case "panel.open":
            try unwrap(MuxyAPI.Panels.open(
                extensionID: context.extensionID,
                panelID: stringArg(args, "panel"),
                data: panelData(args),
                toggle: false
            ))
            return NSNull()
        case "panel.toggle":
            try unwrap(MuxyAPI.Panels.open(
                extensionID: context.extensionID,
                panelID: stringArg(args, "panel"),
                data: panelData(args),
                toggle: true
            ))
            return NSNull()
        case "panel.close":
            try unwrap(MuxyAPI.Panels.close(
                extensionID: context.extensionID,
                panelID: stringArg(args, "panel")
            ))
            return NSNull()
        case "popover.close":
            try unwrap(MuxyAPI.Popovers.close(extensionID: context.extensionID))
            return NSNull()
        case "popover.resize":
            try unwrap(MuxyAPI.Popovers.resize(
                extensionID: context.extensionID,
                width: doubleArg(args, "width"),
                height: doubleArg(args, "height")
            ))
            return NSNull()
        case "exec":
            return try await handleExec(args: args, context: context)
        case "dialog.confirm":
            let request = try ExtensionDialogService.makeConfirmRequest(extensionID: context.extensionID, args: args)
            return try await ExtensionDialogService.confirm(request) ?? NSNull()
        case "dialog.alert":
            let request = try ExtensionDialogService.makeAlertRequest(extensionID: context.extensionID, args: args)
            try await ExtensionDialogService.alert(request)
            return NSNull()
        case "tabs.list":
            return try unwrap(MuxyAPI.Tabs.list(appState: context.appState)).map(tabDict)
        case "tabs.switch":
            try unwrap(MuxyAPI.Tabs.switchTo(
                identifier: stringArg(args, "identifier"),
                appState: context.appState
            ))
            return NSNull()
        case "tabs.new":
            return try unwrap(MuxyAPI.Tabs.new(appState: context.appState))?.uuidString ?? NSNull()
        case "tabs.next":
            try unwrap(MuxyAPI.Tabs.next(appState: context.appState))
            return NSNull()
        case "tabs.previous":
            try unwrap(MuxyAPI.Tabs.previous(appState: context.appState))
            return NSNull()
        case "tabs.open":
            try await unwrap(MuxyAPI.Tabs.open(
                decodeOpenTabRequest(args),
                appState: context.appState,
                callingExtensionID: context.extensionID
            ))
            return NSNull()
        case "panes.list":
            return MuxyAPI.Panes.list(appState: context.appState).map(paneDict)
        case "panes.send":
            try await unwrap(MuxyAPI.Panes.send(
                paneIDString: stringArg(args, "paneID"),
                text: stringArg(args, "text"),
                appState: context.appState,
                extensionID: context.extensionID
            ))
            return NSNull()
        case "panes.sendKeys":
            try await unwrap(MuxyAPI.Panes.sendKeys(
                paneIDString: stringArg(args, "paneID"),
                key: stringArg(args, "key"),
                appState: context.appState,
                extensionID: context.extensionID
            ))
            return NSNull()
        case "panes.readScreen":
            let lines = (args["lines"] as? Int) ?? 50
            return try await unwrap(MuxyAPI.Panes.readScreen(
                paneIDString: stringArg(args, "paneID"),
                lines: lines,
                appState: context.appState,
                extensionID: context.extensionID
            ))
        case "panes.close":
            try unwrap(MuxyAPI.Panes.close(
                paneIDString: stringArg(args, "paneID"),
                appState: context.appState
            ))
            return NSNull()
        case "panes.rename":
            try unwrap(MuxyAPI.Panes.rename(
                paneIDString: stringArg(args, "paneID"),
                title: stringArg(args, "title"),
                appState: context.appState
            ))
            return NSNull()
        case "projects.list":
            guard let projectStore = context.projectStore else { throw APIError.projectStoreUnavailable }
            return MuxyAPI.Projects.list(appState: context.appState, projectStore: projectStore).map(projectDict)
        case "projects.switch":
            guard let projectStore = context.projectStore,
                  let worktreeStore = context.worktreeStore
            else { throw APIError.projectStoreUnavailable }
            try unwrap(MuxyAPI.Projects.switchTo(
                identifier: stringArg(args, "identifier"),
                appState: context.appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            ))
            return NSNull()
        case "worktrees.list":
            guard let projectStore = context.projectStore,
                  let worktreeStore = context.worktreeStore
            else { throw APIError.worktreeStoreUnavailable }
            return try unwrap(MuxyAPI.Worktrees.list(
                projectIdentifier: args["project"] as? String,
                appState: context.appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            )).map(worktreeDict)
        case "worktrees.switch":
            guard let projectStore = context.projectStore,
                  let worktreeStore = context.worktreeStore
            else { throw APIError.worktreeStoreUnavailable }
            try unwrap(MuxyAPI.Worktrees.switchTo(
                identifier: stringArg(args, "identifier"),
                projectIdentifier: args["project"] as? String,
                appState: context.appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            ))
            return NSNull()
        case "worktrees.refresh":
            guard let projectStore = context.projectStore,
                  let worktreeStore = context.worktreeStore
            else { throw APIError.worktreeStoreUnavailable }
            let result = try await unwrap(MuxyAPI.Worktrees.refresh(
                projectIdentifier: args["project"] as? String,
                appState: context.appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            ))
            return ["count": result.count]
        default:
            throw APIError.invalidArguments("unknown verb \(verb)")
        }
    }

    private static func handleExec(args: [String: Any], context: Context) async throws -> Any {
        let request = try ExtensionBridgeShared.decodeExecRequest(args)
        let defaultCwd = ExtensionBridgeShared.activeWorktreePath(
            appState: context.appState,
            worktreeStore: context.worktreeStore
        )
        let result = try await ExtensionCommandExecutor.exec(
            request: request,
            extensionID: context.extensionID,
            defaultCwd: defaultCwd
        )
        return ExtensionBridgeShared.encodeExecResult(result)
    }

    private static func handleNotify(args: [String: Any], context: Context) async throws -> Any {
        let title = (args["title"] as? String) ?? ""
        let body = (args["body"] as? String) ?? ""
        guard !title.isEmpty || !body.isEmpty else {
            throw APIError.invalidArguments("notification requires title or body")
        }
        let source = MuxyNotification.Source.aiProvider(context.extensionID)
        if let paneIDString = args["paneID"] as? String, let paneID = UUID(uuidString: paneIDString) {
            NotificationStore.shared.add(
                paneID: paneID,
                source: source,
                title: title,
                body: body,
                appState: context.appState
            )
            return NSNull()
        }
        guard let projectID = context.appState.activeProjectID,
              let key = context.appState.activeWorktreeKey(for: projectID),
              let root = context.appState.workspaceRoots[key]
        else { throw APIError.noActiveProject }
        for area in root.allAreas() {
            for tab in area.tabs where tab.content.pane != nil {
                let navigationContext = NavigationContext(
                    projectID: key.projectID,
                    worktreeID: key.worktreeID,
                    worktreePath: area.projectPath,
                    areaID: area.id,
                    tabID: tab.id
                )
                NotificationStore.shared.addWithContext(
                    context: navigationContext,
                    source: source,
                    title: title,
                    body: body,
                    appState: context.appState
                )
                return NSNull()
            }
        }
        throw APIError.noFocusedArea
    }

    private static func stringArg(_ args: [String: Any], _ key: String) throws -> String {
        if let value = args[key] as? String { return value }
        throw APIError.invalidArguments("missing argument '\(key)'")
    }

    private static func doubleArg(_ args: [String: Any], _ key: String) throws -> Double {
        if let value = args[key] as? Double { return value }
        if let value = args[key] as? Int { return Double(value) }
        if let value = args[key] as? NSNumber { return value.doubleValue }
        throw APIError.invalidArguments("missing argument '\(key)'")
    }

    private static func unwrap<T>(_ result: Result<T, APIError>) throws -> T {
        switch result {
        case let .success(value): return value
        case let .failure(error): throw error
        }
    }

    private static func panelData(_ args: [String: Any]) -> ExtensionJSON? {
        guard let raw = args["data"] else { return nil }
        guard let data = try? JSONSerialization.data(withJSONObject: raw) else { return nil }
        return try? JSONDecoder().decode(ExtensionJSON.self, from: data)
    }

    private static func decodeOpenTabRequest(_ args: [String: Any]) throws -> OpenTabRequest {
        let data = try JSONSerialization.data(withJSONObject: args)
        do {
            return try JSONDecoder().decode(OpenTabRequest.self, from: data)
        } catch {
            throw APIError.invalidArguments("invalid open tab request: \(error.localizedDescription)")
        }
    }

    private static func tabDict(_ tab: TabInfo) -> [String: Any] {
        [
            "index": tab.index,
            "id": tab.id.uuidString,
            "kind": tab.kind.rawValue,
            "title": tab.title,
            "isActive": tab.isActive,
        ]
    }

    private static func paneDict(_ pane: PaneInfo) -> [String: Any] {
        [
            "id": pane.id.uuidString,
            "title": pane.title,
            "workingDirectory": pane.workingDirectory,
            "isFocused": pane.isFocused,
        ]
    }

    private static func projectDict(_ project: ProjectInfo) -> [String: Any] {
        [
            "id": project.id.uuidString,
            "name": project.name,
            "path": project.path,
            "isActive": project.isActive,
        ]
    }

    private static func worktreeDict(_ worktree: WorktreeInfo) -> [String: Any] {
        [
            "id": worktree.id.uuidString,
            "name": worktree.name,
            "path": worktree.path,
            "branch": worktree.branch ?? NSNull(),
            "isActive": worktree.isActive,
        ]
    }
}
