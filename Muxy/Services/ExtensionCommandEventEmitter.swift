import Foundation

@MainActor
enum ExtensionCommandEventEmitter {
    static func emit(paneID: UUID, command: String, workingDirectory: String?) {
        let trimmedCommand = command.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedCommand.isEmpty else { return }
        guard let appState = NotificationStore.shared.appState,
              let located = appState.locateTab(forPane: paneID)
        else { return }

        var payload: [String: String] = [
            "command": trimmedCommand,
            "paneID": paneID.uuidString,
            "projectID": located.worktreeKey.projectID.uuidString,
            "worktreeID": located.worktreeKey.worktreeID.uuidString,
            "areaID": located.areaID.uuidString,
            "tabID": located.tabID.uuidString,
        ]
        if let cwd = workingDirectory ?? located.pane.currentWorkingDirectory {
            payload["cwd"] = cwd
        }

        NotificationSocketServer.shared.broadcast(event: ExtensionEvent(
            name: ExtensionEventName.commandExecuted,
            payload: payload
        ))
    }
}
