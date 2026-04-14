import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "NotificationSocketServer")

final class NotificationSocketServer: @unchecked Sendable {
    static let shared = NotificationSocketServer()

    private var serverFD: Int32 = -1
    private var acceptSource: DispatchSourceRead?
    private let queue = DispatchQueue(label: "app.muxy.notificationSocket")

    static var socketPath: String {
        "/tmp/muxy-\(getuid()).sock"
    }

    private init() {}

    func start() {
        queue.async { [weak self] in
            self?.startListening()
        }
    }

    func stop() {
        queue.async { [weak self] in
            self?.cleanup()
        }
    }

    private func startListening() {
        let path = Self.socketPath
        unlink(path)

        serverFD = socket(AF_UNIX, SOCK_STREAM, 0)
        guard serverFD >= 0 else {
            logger.error("Failed to create socket: \(String(cString: strerror(errno)))")
            return
        }

        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        withUnsafeMutablePointer(to: &addr.sun_path) { ptr in
            let bound = ptr.withMemoryRebound(to: CChar.self, capacity: 104) { $0 }
            _ = path.withCString { strncpy(bound, $0, 103) }
        }

        let bindResult = withUnsafePointer(to: &addr) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                bind(serverFD, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard bindResult == 0 else {
            logger.error("Failed to bind socket: \(String(cString: strerror(errno)))")
            close(serverFD)
            serverFD = -1
            return
        }

        chmod(path, 0o600)

        guard listen(serverFD, 5) == 0 else {
            logger.error("Failed to listen on socket: \(String(cString: strerror(errno)))")
            close(serverFD)
            serverFD = -1
            return
        }

        let source = DispatchSource.makeReadSource(fileDescriptor: serverFD, queue: queue)
        source.setEventHandler { [weak self] in
            self?.acceptConnection()
        }
        source.setCancelHandler { [weak self] in
            guard let self, self.serverFD >= 0 else { return }
            close(self.serverFD)
            self.serverFD = -1
            unlink(path)
        }
        acceptSource = source
        source.resume()

        logger.info("Notification socket listening at \(path)")
    }

    private func acceptConnection() {
        let clientFD = accept(serverFD, nil, nil)
        guard clientFD >= 0 else { return }

        queue.async { [weak self] in
            self?.handleClient(clientFD)
        }
    }

    private func handleClient(_ fd: Int32) {
        defer { close(fd) }

        var data = Data()
        var buffer = [UInt8](repeating: 0, count: 4096)
        while true {
            let bytesRead = read(fd, &buffer, buffer.count)
            guard bytesRead > 0 else { break }
            data.append(contentsOf: buffer[0 ..< bytesRead])
            if bytesRead < buffer.count { break }
        }

        guard !data.isEmpty else { return }

        for line in data.split(separator: UInt8(ascii: "\n")) {
            processMessage(Data(line))
        }
    }

    private func processMessage(_ data: Data) {
        guard let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            logger.warning("Invalid JSON received on notification socket")
            return
        }

        guard let type = json["type"] as? String else { return }
        let title = json["title"] as? String ?? ""
        let body = json["body"] as? String ?? ""
        let paneIDString = json["paneID"] as? String

        DispatchQueue.main.async {
            self.dispatchNotification(type: type, title: title, body: body, paneIDString: paneIDString)
        }
    }

    @MainActor
    private func dispatchNotification(type: String, title: String, body: String, paneIDString: String?) {
        guard let appState = SystemNotificationService.shared.appState else { return }

        let source: MuxyNotification.Source = switch type {
        case "claude_hook": .claudeHook
        default: .socket
        }

        if let paneIDString, let paneID = UUID(uuidString: paneIDString) {
            NotificationStore.shared.add(
                paneID: paneID,
                source: source,
                title: title,
                body: body,
                appState: appState
            )
            return
        }

        guard let projectID = appState.activeProjectID,
              let key = appState.activeWorktreeKey(for: projectID),
              let context = findFirstPaneContext(key: key, appState: appState)
        else { return }

        NotificationStore.shared.addWithContext(
            context: context,
            source: source,
            title: title,
            body: body,
            appState: appState
        )
    }

    @MainActor
    private func findFirstPaneContext(
        key: WorktreeKey,
        appState: AppState
    ) -> NavigationContext? {
        guard let root = appState.workspaceRoots[key] else { return nil }
        for area in root.allAreas() {
            for tab in area.tabs {
                guard tab.content.pane != nil else { continue }
                let path = NotificationStore.shared.worktreeStore?.worktree(
                    projectID: key.projectID,
                    worktreeID: key.worktreeID
                )?.path ?? area.projectPath
                return NavigationContext(
                    projectID: key.projectID,
                    worktreeID: key.worktreeID,
                    worktreePath: path,
                    areaID: area.id,
                    tabID: tab.id
                )
            }
        }
        return nil
    }

    private func cleanup() {
        acceptSource?.cancel()
        acceptSource = nil
    }
}
