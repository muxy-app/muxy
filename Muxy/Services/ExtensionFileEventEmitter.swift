import Foundation

final class ExtensionFileEventEmitter: @unchecked Sendable {
    static let shared = ExtensionFileEventEmitter()

    private let window: TimeInterval = 0.3
    private let lock = NSLock()
    private var lastEmitted: [String: TimeInterval] = [:]

    static func emit(paths: [String], projectPath: String) {
        shared.emit(paths: paths, projectPath: projectPath)
    }

    func emit(paths: [String], projectPath: String) {
        let now = ProcessInfo.processInfo.systemUptime
        let fresh = deduplicate(paths: paths, projectPath: projectPath, now: now)
        guard !fresh.isEmpty else { return }

        let server = NotificationSocketServer.shared
        for path in fresh {
            server.broadcast(event: ExtensionEvent(
                name: ExtensionEventName.fileChanged,
                payload: [
                    "path": path,
                    "projectPath": projectPath,
                ]
            ))
        }
    }

    private func deduplicate(paths: [String], projectPath: String, now: TimeInterval) -> [String] {
        lock.lock()
        defer { lock.unlock() }

        lastEmitted = lastEmitted.filter { now - $0.value < window }

        var fresh: [String] = []
        for path in paths {
            let key = "\(projectPath)\u{0}\(path)"
            if let last = lastEmitted[key], now - last < window { continue }
            lastEmitted[key] = now
            fresh.append(path)
        }
        return fresh
    }
}
