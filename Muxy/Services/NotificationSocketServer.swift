import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "NotificationSocketServer")

final class NotificationSocketServer: @unchecked Sendable {
    static let shared = NotificationSocketServer()

    private var serverFD: Int32 = -1
    private var acceptSource: DispatchSourceRead?
    private let queue = DispatchQueue(label: "app.muxy.notificationSocket")
    var openProjectHandler: (@Sendable (String) -> Void)?
    var commandHandler: (@Sendable (String) async -> String)?

    static var socketPath: String {
        MuxyFileStorage.appSupportDirectory()
            .appendingPathComponent("muxy.sock")
            .path
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

        chmod(path, mode_t(FilePermissions.privateFile))

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

    private static let maxMessageSize = 65536

    private static let commandNames = [
        "split-right", "split-down", "send", "send-keys",
        "read-screen", "close-pane", "rename-pane", "list-panes",
    ]

    private func handleClient(_ fd: Int32) {
        defer { close(fd) }

        var data = Data()
        var buffer = [UInt8](repeating: 0, count: 4096)
        while true {
            let bytesRead = read(fd, &buffer, buffer.count)
            if bytesRead <= 0 { break }
            data.append(contentsOf: buffer[0 ..< bytesRead])
            if data.count > Self.maxMessageSize {
                logger.warning("Client exceeded max message size (\(Self.maxMessageSize) bytes), dropping")
                return
            }
        }

        guard !data.isEmpty,
              let message = String(data: data, encoding: .utf8)
        else { return }

        let commandMessage = message.trimmingCharacters(in: .whitespacesAndNewlines)
        if isCommandMessage(commandMessage) {
            let response = processCommand(commandMessage)
            writeResponse(fd: fd, text: response)
            return
        }

        for line in data.split(separator: UInt8(ascii: "\n")) {
            processMessage(Data(line))
        }
    }

    private func isCommandMessage(_ message: String) -> Bool {
        guard let command = message.split(separator: "|", maxSplits: 1, omittingEmptySubsequences: false).first else {
            return false
        }
        return Self.commandNames.contains(String(command))
    }

    private func processCommand(_ message: String) -> String {
        guard let handler = commandHandler else {
            return "error:no handler registered"
        }

        let semaphore = DispatchSemaphore(value: 0)
        nonisolated(unsafe) var response = "error:timeout"

        Task { @Sendable in
            response = await handler(message)
            semaphore.signal()
        }

        guard semaphore.wait(timeout: .now() + .seconds(5)) == .success else {
            return "error:timeout"
        }
        return response
    }

    private func writeResponse(fd: Int32, text: String) {
        let data = Data(text.utf8)
        data.withUnsafeBytes { buffer in
            guard let ptr = buffer.baseAddress else { return }
            var offset = 0
            while offset < buffer.count {
                let written = Darwin.write(fd, ptr.advanced(by: offset), buffer.count - offset)
                if written <= 0 { break }
                offset += written
            }
        }
    }

    private func processMessage(_ data: Data) {
        guard let message = String(data: data, encoding: .utf8) else { return }
        let prefix = "open-project|"
        if message.hasPrefix(prefix) {
            let path = String(message.dropFirst(prefix.count))
            var isDirectory: ObjCBool = false
            guard !path.isEmpty,
                  FileManager.default.fileExists(atPath: path, isDirectory: &isDirectory),
                  isDirectory.boolValue
            else {
                logger.warning("Ignoring open-project for invalid path")
                return
            }
            logger.info("Received open-project request via socket")
            openProjectHandler?(path)
            return
        }

        if message == "split-right" || message.hasPrefix("split-right|") {
            logger.info("Received legacy split-right request via socket")
            return
        }

        if message == "split-down" || message.hasPrefix("split-down|") {
            logger.info("Received legacy split-down request via socket")
            return
        }

        let parts = message.split(separator: "|", maxSplits: 3).map(String.init)
        guard parts.count >= 3 else {
            logger.warning("Invalid message on notification socket: expected type|paneID|title|body")
            return
        }

        let type = parts[0]
        let paneIDString = parts[1]
        let rawTitle = parts[2]
        let title = rawTitle.isEmpty ? "Task completed!" : rawTitle
        let body = parts.count > 3 ? parts[3] : ""

        DispatchQueue.main.async { [weak self] in
            self?.dispatchNotification(type: type, title: title, body: body, paneIDString: paneIDString)
        }
    }

    @MainActor
    private func dispatchNotification(type: String, title: String, body: String, paneIDString: String?) {
        guard let appState = NotificationStore.shared.appState else { return }

        let source = AIProviderRegistry.shared.notificationSource(for: type)

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
