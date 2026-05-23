import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "SocketCommandHandler")

@MainActor
enum SocketCommandHandler {
    static func handleRequest(_ message: String, appState: AppState) async -> String {
        let parts = message.components(separatedBy: "|")
        guard let cmd = parts.first else {
            return "error:empty command"
        }

        switch cmd {
        case "split-right":
            let request = parseSplitRequest(parts: parts)
            return handleSplit(direction: .horizontal, command: request.command, fromPane: request.fromPane, appState: appState)
        case "split-down":
            let request = parseSplitRequest(parts: parts)
            return handleSplit(direction: .vertical, command: request.command, fromPane: request.fromPane, appState: appState)
        case "send":
            guard parts.count >= 3 else { return "error:usage send|paneID|text" }
            return await handleSend(paneIDStr: parts[1], text: parts.dropFirst(2).joined(separator: "|"), appState: appState)
        case "send-keys":
            guard parts.count >= 3 else { return "error:usage send-keys|paneID|key" }
            return await handleSendKeys(paneIDStr: parts[1], key: parts[2], appState: appState)
        case "read-screen":
            guard parts.count >= 2 else { return "error:usage read-screen|paneID[|lines]" }
            let lines = parts.count >= 3 ? Int(parts[2]) ?? 50 : 50
            return await handleReadScreen(paneIDStr: parts[1], lines: lines, appState: appState)
        case "close-pane":
            guard parts.count >= 2 else { return "error:usage close-pane|paneID" }
            return handleClosePane(paneIDStr: parts[1], appState: appState)
        case "rename-pane":
            guard parts.count >= 3 else { return "error:usage rename-pane|paneID|title" }
            return handleRenamePane(paneIDStr: parts[1], title: parts.dropFirst(2).joined(separator: "|"), appState: appState)
        case "list-panes":
            return handleListPanes(appState: appState)
        default:
            return "error:unknown command \(cmd)"
        }
    }

    private static func parseSplitRequest(parts: [String]) -> (fromPane: String?, command: String?) {
        guard parts.count >= 2 else { return (nil, nil) }
        let firstValue = parts[1]
        let firstValueIsPane = firstValue.isEmpty || UUID(uuidString: firstValue) != nil
        if firstValueIsPane {
            let command = parts.count >= 3 ? parts.dropFirst(2).joined(separator: "|") : nil
            return (firstValue, command)
        }
        if parts.count >= 3, let fromPane = parts.last, UUID(uuidString: fromPane) != nil {
            return (fromPane, parts.dropFirst(1).dropLast().joined(separator: "|"))
        }
        return (nil, parts.dropFirst(1).joined(separator: "|"))
    }

    private static func handleSplit(direction: SplitDirection, command: String?, fromPane: String?, appState: AppState) -> String {
        let projectID: UUID
        let areaID: UUID

        if let fromPane, let paneID = UUID(uuidString: fromPane),
           let loc = locateTab(paneID: paneID, appState: appState)
        {
            projectID = loc.key.projectID
            areaID = loc.areaID
        } else {
            guard let activeID = appState.activeProjectID else {
                return "error:no active project"
            }
            guard let area = appState.focusedArea(for: activeID) else {
                return "error:no focused area"
            }
            projectID = activeID
            areaID = area.id
        }

        let trimmedCommand = command?.trimmingCharacters(in: .whitespacesAndNewlines)
        let finalCommand = (trimmedCommand?.isEmpty ?? true) ? nil : trimmedCommand

        let existingPaneIDs = collectAllPaneIDs(appState: appState)

        appState.dispatch(.splitArea(.init(
            projectID: projectID,
            areaID: areaID,
            direction: direction,
            position: .second,
            command: finalCommand
        )))

        let newPaneIDs = collectAllPaneIDs(appState: appState)
        let added = newPaneIDs.subtracting(existingPaneIDs)

        guard let newPaneID = added.first else {
            return "error:split succeeded but could not determine new pane ID"
        }

        return newPaneID.uuidString
    }

    private static func handleSend(paneIDStr: String, text: String, appState: AppState) async -> String {
        guard let paneID = UUID(uuidString: paneIDStr) else {
            return "error:invalid pane ID"
        }
        guard let view = await waitForView(paneID: paneID, appState: appState) else {
            return "error:pane not found \(paneIDStr)"
        }

        view.sendText(text)
        return "ok"
    }

    private static func handleSendKeys(paneIDStr: String, key: String, appState: AppState) async -> String {
        guard let paneID = UUID(uuidString: paneIDStr) else {
            return "error:invalid pane ID"
        }
        guard let view = await waitForView(paneID: paneID, appState: appState) else {
            return "error:pane not found \(paneIDStr)"
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
            return "error:unsupported key \(key)"
        }

        view.sendRemoteBytes(bytes)
        return "ok"
    }

    private static func handleReadScreen(paneIDStr: String, lines: Int, appState: AppState) async -> String {
        guard let paneID = UUID(uuidString: paneIDStr) else {
            return "error:invalid pane ID"
        }
        let clampedLines = min(max(lines, 1), 500)

        guard let view = await waitForView(paneID: paneID, appState: appState) else {
            return "error:pane not found \(paneIDStr)"
        }

        return view.readScreenText(lastLines: clampedLines)
    }

    private static func handleClosePane(paneIDStr: String, appState: AppState) -> String {
        guard let paneID = UUID(uuidString: paneIDStr) else {
            return "error:invalid pane ID"
        }

        guard let loc = locateTab(paneID: paneID, appState: appState) else {
            return "error:pane not found \(paneIDStr)"
        }

        appState.dispatch(.closeTab(projectID: loc.key.projectID, areaID: loc.areaID, tabID: loc.tabID))
        return "ok"
    }

    private static func handleRenamePane(paneIDStr: String, title: String, appState: AppState) -> String {
        guard let paneID = UUID(uuidString: paneIDStr) else {
            return "error:invalid pane ID"
        }

        guard let loc = locateTab(paneID: paneID, appState: appState) else {
            return "error:pane not found \(paneIDStr)"
        }

        for (_, root) in appState.workspaceRoots {
            guard let area = root.findArea(id: loc.areaID) else { continue }
            area.setCustomTitle(loc.tabID, title: title)
            return "ok"
        }

        return "error:could not rename pane"
    }

    private static func handleListPanes(appState: AppState) -> String {
        var lines: [String] = []
        for (key, root) in appState.workspaceRoots {
            let focusedAreaID = appState.focusedAreaID(for: key.projectID)
            for area in root.allAreas() {
                for tab in area.tabs {
                    guard let pane = tab.content.pane else { continue }
                    let isFocused = area.id == focusedAreaID && tab.id == area.activeTabID
                    let title = tab.customTitle ?? pane.title
                    let cwd = pane.currentWorkingDirectory ?? pane.projectPath
                    lines.append("\(pane.id.uuidString)\t\(title)\t\(cwd)\t\(isFocused)")
                }
            }
        }
        return lines.joined(separator: "\n")
    }

    private static func waitForView(
        paneID: UUID,
        appState: AppState? = nil,
        timeout: Duration = .seconds(3)
    ) async -> GhosttyTerminalNSView? {
        if let view = TerminalViewRegistry.shared.existingView(for: paneID) {
            return view
        }
        if let appState, locateTab(paneID: paneID, appState: appState) == nil {
            return nil
        }
        let deadline = ContinuousClock.now + timeout
        while ContinuousClock.now < deadline {
            if let view = TerminalViewRegistry.shared.existingView(for: paneID) {
                return view
            }
            try? await Task.sleep(for: .milliseconds(50))
        }
        return nil
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
