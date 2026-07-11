import Foundation

@MainActor
enum ExtensionLifecycleEvents {
    static func panelOpened(extensionID: String, panelID: String) {
        broadcast(name: ExtensionEventName.panelOpened, extensionID: extensionID, surfaceID: panelID, key: "panelID")
    }

    static func panelClosed(extensionID: String, panelID: String) {
        broadcast(name: ExtensionEventName.panelClosed, extensionID: extensionID, surfaceID: panelID, key: "panelID")
    }

    static func popoverOpened(extensionID: String, popoverID: String) {
        broadcast(name: ExtensionEventName.popoverOpened, extensionID: extensionID, surfaceID: popoverID, key: "popoverID")
    }

    static func popoverClosed(extensionID: String, popoverID: String) {
        broadcast(name: ExtensionEventName.popoverClosed, extensionID: extensionID, surfaceID: popoverID, key: "popoverID")
    }

    static func modalOpened(extensionID: String, modalID: String) {
        broadcast(name: ExtensionEventName.modalOpened, extensionID: extensionID, surfaceID: modalID, key: "modalID")
    }

    static func modalClosed(extensionID: String, modalID: String) {
        broadcast(name: ExtensionEventName.modalClosed, extensionID: extensionID, surfaceID: modalID, key: "modalID")
    }

    private static func broadcast(name: String, extensionID: String, surfaceID: String, key: String) {
        NotificationSocketServer.shared.broadcast(event: ExtensionEvent(
            name: name,
            payload: ["extensionID": extensionID, key: surfaceID]
        ))
    }
}
