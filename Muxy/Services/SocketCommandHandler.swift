import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "SocketCommandHandler")

@MainActor
enum SocketCommandHandler {
    static func handleRequest(_ data: Data, appState: AppState) async -> Data {
        guard let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let cmd = json["cmd"] as? String
        else {
            return NotificationSocketServer.jsonResponse(["ok": false, "error": "Invalid JSON request"])
        }
        let result = handle(cmd: cmd, params: json, appState: appState)
        return NotificationSocketServer.jsonResponse(result)
    }

    static func handle(
        cmd: String,
        params: [String: Any],
        appState: AppState
    ) -> [String: Any] {
        switch cmd {
        case "split":
            handleSplit(params: params, appState: appState)
        case "send":
            handleSend(params: params)
        case "send-keys":
            handleSendKeys(params: params)
        case "read-screen":
            handleReadScreen(params: params)
        case "close-pane":
            handleClosePane(params: params, appState: appState)
        case "rename-pane":
            handleRenamePane(params: params, appState: appState)
        case "list-panes":
            handleListPanes(appState: appState)
        default:
            ["ok": false, "error": "Unknown command: \(cmd)"]
        }
    }

    private static func handleSplit(params: [String: Any], appState: AppState) -> [String: Any] {
        guard let projectID = appState.activeProjectID else {
            return ["ok": false, "error": "No active project"]
        }
        guard let area = appState.focusedArea(for: projectID) else {
            return ["ok": false, "error": "No focused area"]
        }

        let directionStr = params["direction"] as? String ?? "right"
        let direction: SplitDirection = directionStr == "down" ? .vertical : .horizontal
        let command = params["command"] as? String

        let existingPaneIDs = collectAllPaneIDs(appState: appState)

        appState.dispatch(.splitArea(.init(
            projectID: projectID,
            areaID: area.id,
            direction: direction,
            position: .second,
            command: command
        )))

        let newPaneIDs = collectAllPaneIDs(appState: appState)
        let added = newPaneIDs.subtracting(existingPaneIDs)

        guard let newPaneID = added.first else {
            return ["ok": false, "error": "Split succeeded but could not determine new pane ID"]
        }

        return ["ok": true, "paneID": newPaneID.uuidString]
    }

    private static func handleSend(params: [String: Any]) -> [String: Any] {
        guard let paneIDStr = params["pane"] as? String,
              let paneID = UUID(uuidString: paneIDStr)
        else {
            return ["ok": false, "error": "Invalid or missing pane ID"]
        }
        guard let text = params["text"] as? String else {
            return ["ok": false, "error": "Missing text parameter"]
        }
        guard let view = TerminalViewRegistry.shared.existingView(for: paneID) else {
            return ["ok": false, "error": "Pane not found: \(paneIDStr)"]
        }

        let parts = text.components(separatedBy: "\n")
        for (index, part) in parts.enumerated() {
            if !part.isEmpty {
                view.sendText(part)
            }
            if index < parts.count - 1 {
                view.sendRemoteBytes(Data([0x0D]))
            }
        }
        return ["ok": true]
    }

    private static func handleSendKeys(params: [String: Any]) -> [String: Any] {
        guard let paneIDStr = params["pane"] as? String,
              let paneID = UUID(uuidString: paneIDStr)
        else {
            return ["ok": false, "error": "Invalid or missing pane ID"]
        }
        guard let key = params["key"] as? String else {
            return ["ok": false, "error": "Missing key parameter"]
        }
        guard let view = TerminalViewRegistry.shared.existingView(for: paneID) else {
            return ["ok": false, "error": "Pane not found: \(paneIDStr)"]
        }

        let bytes: Data
        switch key.lowercased() {
        case "escape",
             "esc":
            bytes = Data([0x1B])
        case "enter",
             "return":
            bytes = Data([0x0D])
        case "tab":
            bytes = Data([0x09])
        case "ctrl+c",
             "ctrl-c":
            bytes = Data([0x03])
        case "ctrl+d",
             "ctrl-d":
            bytes = Data([0x04])
        case "ctrl+z",
             "ctrl-z":
            bytes = Data([0x1A])
        case "backspace":
            bytes = Data([0x7F])
        default:
            return ["ok": false, "error": "Unsupported key: \(key)"]
        }

        view.sendRemoteBytes(bytes)
        return ["ok": true]
    }

    private static func handleReadScreen(params: [String: Any]) -> [String: Any] {
        guard let paneIDStr = params["pane"] as? String,
              let paneID = UUID(uuidString: paneIDStr)
        else {
            return ["ok": false, "error": "Invalid or missing pane ID"]
        }
        let lines = params["lines"] as? Int ?? 50
        let clampedLines = min(max(lines, 1), 500)

        guard let view = TerminalViewRegistry.shared.existingView(for: paneID) else {
            return ["ok": false, "error": "Pane not found: \(paneIDStr)"]
        }

        let content = view.readScreenText(lastLines: clampedLines)
        return ["ok": true, "content": content]
    }

    private static func handleClosePane(params: [String: Any], appState: AppState) -> [String: Any] {
        guard let paneIDStr = params["pane"] as? String,
              let paneID = UUID(uuidString: paneIDStr)
        else {
            return ["ok": false, "error": "Invalid or missing pane ID"]
        }

        guard let loc = locateTab(paneID: paneID, appState: appState) else {
            return ["ok": false, "error": "Pane not found: \(paneIDStr)"]
        }

        appState.dispatch(.closeTab(projectID: loc.key.projectID, areaID: loc.areaID, tabID: loc.tabID))
        return ["ok": true]
    }

    private static func handleRenamePane(params: [String: Any], appState: AppState) -> [String: Any] {
        guard let paneIDStr = params["pane"] as? String,
              let paneID = UUID(uuidString: paneIDStr)
        else {
            return ["ok": false, "error": "Invalid or missing pane ID"]
        }
        guard let title = params["title"] as? String else {
            return ["ok": false, "error": "Missing title parameter"]
        }

        guard let loc = locateTab(paneID: paneID, appState: appState) else {
            return ["ok": false, "error": "Pane not found: \(paneIDStr)"]
        }

        for (_, root) in appState.workspaceRoots {
            guard let area = root.findArea(id: loc.areaID) else { continue }
            area.setCustomTitle(loc.tabID, title: title)
            return ["ok": true]
        }

        return ["ok": false, "error": "Could not rename pane"]
    }

    private static func handleListPanes(appState: AppState) -> [String: Any] {
        var panes: [[String: Any]] = []
        for (key, root) in appState.workspaceRoots {
            let focusedAreaID = appState.focusedAreaID(for: key.projectID)
            for area in root.allAreas() {
                for tab in area.tabs {
                    guard let pane = tab.content.pane else { continue }
                    let isFocused = area.id == focusedAreaID && tab.id == area.activeTabID
                    panes.append([
                        "id": pane.id.uuidString,
                        "title": tab.customTitle ?? pane.title,
                        "cwd": pane.currentWorkingDirectory ?? pane.projectPath,
                        "projectID": key.projectID.uuidString,
                        "focused": isFocused,
                    ])
                }
            }
        }
        return ["ok": true, "panes": panes]
    }

    private static func collectAllPaneIDs(appState: AppState) -> Set<UUID> {
        var ids = Set<UUID>()
        for (_, root) in appState.workspaceRoots {
            for area in root.allAreas() {
                for tab in area.tabs {
                    if let pane = tab.content.pane {
                        ids.insert(pane.id)
                    }
                }
            }
        }
        return ids
    }

    private struct PaneLocation {
        let key: WorktreeKey
        let areaID: UUID
        let tabID: UUID
    }

    private static func locateTab(paneID: UUID, appState: AppState) -> PaneLocation? {
        for (key, root) in appState.workspaceRoots {
            for area in root.allAreas() {
                for tab in area.tabs where tab.content.pane?.id == paneID {
                    return PaneLocation(key: key, areaID: area.id, tabID: tab.id)
                }
            }
        }
        return nil
    }
}
