import Foundation
import os
import WebKit

private let logger = Logger(subsystem: "app.muxy", category: "ExtensionBridge")

@MainActor
final class ExtensionBridgeHandler: NSObject, WKScriptMessageHandlerWithReply {
    private let extensionID: String
    private weak var appState: AppState?
    private weak var projectStore: ProjectStore?
    private weak var worktreeStore: WorktreeStore?

    init(
        extensionID: String,
        appState: AppState,
        projectStore: ProjectStore?,
        worktreeStore: WorktreeStore?
    ) {
        self.extensionID = extensionID
        self.appState = appState
        self.projectStore = projectStore
        self.worktreeStore = worktreeStore
    }

    func userContentController(
        _: WKUserContentController,
        didReceive message: WKScriptMessage,
        replyHandler: @escaping @MainActor (Any?, String?) -> Void
    ) {
        let body = message.body
        Task { @MainActor in
            let reply = await self.dispatch(body)
            replyHandler(reply, nil)
        }
    }

    private func dispatch(_ body: Any) async -> [String: Any] {
        guard let payload = body as? [String: Any],
              let verb = payload["verb"] as? String,
              let requestID = payload["requestID"] as? String
        else {
            return ["ok": false, "error": "invalid message"]
        }
        let args = (payload["args"] as? [String: Any]) ?? [:]

        if let required = Self.verbPermissions[verb],
           !ExtensionStore.shared.extensionHasPermission(id: extensionID, permission: required)
        {
            return [
                "requestID": requestID,
                "ok": false,
                "error": "permission denied (\(required.rawValue))",
            ]
        }

        guard let appState else {
            return ["requestID": requestID, "ok": false, "error": "app state unavailable"]
        }

        do {
            let value = try await handle(verb: verb, args: args, appState: appState)
            return ["requestID": requestID, "ok": true, "value": value]
        } catch let error as APIError {
            return ["requestID": requestID, "ok": false, "error": error.message]
        } catch {
            return ["requestID": requestID, "ok": false, "error": error.localizedDescription]
        }
    }

    private static let verbPermissions: [String: ExtensionPermission] = [
        "tabs.list": .tabsRead,
        "tabs.switch": .tabsWrite,
        "tabs.new": .tabsWrite,
        "tabs.next": .tabsWrite,
        "tabs.previous": .tabsWrite,
        "tabs.open": .tabsWrite,
        "panes.list": .panesRead,
        "panes.send": .panesWrite,
        "panes.sendKeys": .panesWrite,
        "panes.readScreen": .panesRead,
        "panes.close": .panesWrite,
        "panes.rename": .panesWrite,
        "projects.list": .projectsRead,
        "projects.switch": .projectsWrite,
        "worktrees.list": .worktreesRead,
        "worktrees.switch": .worktreesWrite,
        "worktrees.refresh": .worktreesWrite,
        "toast": .notificationsWrite,
    ]

    private func handle(verb: String, args: [String: Any], appState: AppState) async throws -> Any {
        switch verb {
        case "toast":
            return try await handleToast(args: args, appState: appState)
        case "tabs.list":
            return try unwrap(MuxyAPI.Tabs.list(appState: appState)).map { tab in
                [
                    "index": tab.index,
                    "id": tab.id.uuidString,
                    "kind": tab.kind.rawValue,
                    "title": tab.title,
                    "isActive": tab.isActive,
                ] as [String: Any]
            }
        case "tabs.switch":
            try unwrap(MuxyAPI.Tabs.switchTo(
                identifier: stringArg(args, "identifier"),
                appState: appState
            ))
            return NSNull()
        case "tabs.new":
            let newID = try unwrap(MuxyAPI.Tabs.new(appState: appState))
            return newID?.uuidString ?? NSNull()
        case "tabs.next":
            try unwrap(MuxyAPI.Tabs.next(appState: appState))
            return NSNull()
        case "tabs.previous":
            try unwrap(MuxyAPI.Tabs.previous(appState: appState))
            return NSNull()
        case "tabs.open":
            let request = try decodeOpenTabRequest(args)
            try unwrap(MuxyAPI.Tabs.open(request, appState: appState))
            return NSNull()
        case "panes.list":
            return MuxyAPI.Panes.list(appState: appState).map { pane in
                [
                    "id": pane.id.uuidString,
                    "title": pane.title,
                    "workingDirectory": pane.workingDirectory,
                    "isFocused": pane.isFocused,
                ] as [String: Any]
            }
        case "panes.send":
            try await unwrap(MuxyAPI.Panes.send(
                paneIDString: stringArg(args, "paneID"),
                text: stringArg(args, "text"),
                appState: appState
            ))
            return NSNull()
        case "panes.sendKeys":
            try await unwrap(MuxyAPI.Panes.sendKeys(
                paneIDString: stringArg(args, "paneID"),
                key: stringArg(args, "key"),
                appState: appState
            ))
            return NSNull()
        case "panes.readScreen":
            let lines = (args["lines"] as? Int) ?? 50
            return try await unwrap(MuxyAPI.Panes.readScreen(
                paneIDString: stringArg(args, "paneID"),
                lines: lines,
                appState: appState
            ))
        case "panes.close":
            try unwrap(MuxyAPI.Panes.close(
                paneIDString: stringArg(args, "paneID"),
                appState: appState
            ))
            return NSNull()
        case "panes.rename":
            try unwrap(MuxyAPI.Panes.rename(
                paneIDString: stringArg(args, "paneID"),
                title: stringArg(args, "title"),
                appState: appState
            ))
            return NSNull()
        case "projects.list":
            guard let projectStore else { throw APIError.projectStoreUnavailable }
            return MuxyAPI.Projects.list(appState: appState, projectStore: projectStore).map { project in
                [
                    "id": project.id.uuidString,
                    "name": project.name,
                    "path": project.path,
                    "isActive": project.isActive,
                ] as [String: Any]
            }
        case "projects.switch":
            guard let projectStore, let worktreeStore else { throw APIError.projectStoreUnavailable }
            try unwrap(MuxyAPI.Projects.switchTo(
                identifier: stringArg(args, "identifier"),
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            ))
            return NSNull()
        case "worktrees.list":
            guard let projectStore, let worktreeStore else { throw APIError.worktreeStoreUnavailable }
            return try unwrap(MuxyAPI.Worktrees.list(
                projectIdentifier: args["project"] as? String,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            )).map { worktree in
                [
                    "id": worktree.id.uuidString,
                    "name": worktree.name,
                    "path": worktree.path,
                    "branch": worktree.branch ?? NSNull(),
                    "isActive": worktree.isActive,
                ] as [String: Any]
            }
        case "worktrees.switch":
            guard let projectStore, let worktreeStore else { throw APIError.worktreeStoreUnavailable }
            try unwrap(MuxyAPI.Worktrees.switchTo(
                identifier: stringArg(args, "identifier"),
                projectIdentifier: args["project"] as? String,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            ))
            return NSNull()
        case "worktrees.refresh":
            guard let projectStore, let worktreeStore else { throw APIError.worktreeStoreUnavailable }
            let result = try await unwrap(MuxyAPI.Worktrees.refresh(
                projectIdentifier: args["project"] as? String,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            ))
            return ["count": result.count]
        default:
            throw APIError.invalidArguments("unknown verb \(verb)")
        }
    }

    private func handleToast(args: [String: Any], appState: AppState) async throws -> Any {
        let title = (args["title"] as? String) ?? ""
        let body = (args["body"] as? String) ?? ""
        guard !title.isEmpty || !body.isEmpty else {
            throw APIError.invalidArguments("toast requires title or body")
        }
        let source = AIProviderRegistry.shared.notificationSource(for: extensionID)
        if let paneIDString = args["paneID"] as? String, let paneID = UUID(uuidString: paneIDString) {
            NotificationStore.shared.add(
                paneID: paneID,
                source: source,
                title: title,
                body: body,
                appState: appState
            )
            return NSNull()
        }
        guard let projectID = appState.activeProjectID,
              let key = appState.activeWorktreeKey(for: projectID),
              let root = appState.workspaceRoots[key]
        else {
            throw APIError.noActiveProject
        }
        for area in root.allAreas() {
            for tab in area.tabs {
                guard tab.content.pane != nil else { continue }
                let context = NavigationContext(
                    projectID: key.projectID,
                    worktreeID: key.worktreeID,
                    worktreePath: area.projectPath,
                    areaID: area.id,
                    tabID: tab.id
                )
                NotificationStore.shared.addWithContext(
                    context: context,
                    source: source,
                    title: title,
                    body: body,
                    appState: appState
                )
                return NSNull()
            }
        }
        throw APIError.noFocusedArea
    }

    private func stringArg(_ args: [String: Any], _ key: String) throws -> String {
        if let value = args[key] as? String { return value }
        throw APIError.invalidArguments("missing argument '\(key)'")
    }

    private func unwrap<T>(_ result: Result<T, APIError>) throws -> T {
        switch result {
        case let .success(value): return value
        case let .failure(error): throw error
        }
    }

    private func decodeOpenTabRequest(_ args: [String: Any]) throws -> OpenTabRequest {
        let data = try JSONSerialization.data(withJSONObject: args)
        do {
            return try JSONDecoder().decode(OpenTabRequest.self, from: data)
        } catch {
            throw APIError.invalidArguments("invalid open tab request: \(error.localizedDescription)")
        }
    }
}
