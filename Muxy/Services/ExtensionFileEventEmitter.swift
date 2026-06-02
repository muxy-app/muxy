import Foundation

enum ExtensionFileEventEmitter {
    static func emit(paths: [String], projectPath: String) {
        let server = NotificationSocketServer.shared
        for path in paths {
            server.broadcast(event: ExtensionEvent(
                name: ExtensionEventName.fileChanged,
                payload: [
                    "path": path,
                    "projectPath": projectPath,
                ]
            ))
        }
    }
}
