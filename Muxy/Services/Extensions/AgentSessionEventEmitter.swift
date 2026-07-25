import Foundation

enum AgentSessionEventEmitter {
    static func emit(paneID: UUID, providerID: String, sessionID: String?, cwd: String) {
        NotificationSocketServer.shared.broadcast(event: ExtensionEvent(
            name: ExtensionEventName.agentSession,
            payload: [
                "paneID": paneID.uuidString,
                "providerID": providerID,
                "sessionID": sessionID ?? "",
                "cwd": cwd,
            ]))
    }
}
