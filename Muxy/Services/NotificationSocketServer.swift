import Foundation
import MuxyShared
import os

private let logger = Logger(subsystem: "app.muxy", category: "NotificationSocketServer")

final class NotificationSocketServer: @unchecked Sendable {
    static let shared = NotificationSocketServer()

    struct ExtensionSnapshotEntry: Equatable {
        let allowedEvents: Set<String>
        let commandEvents: Set<String>
        let permissions: Set<ExtensionPermission>
        let token: String
    }

    struct ExtensionSnapshot: Equatable {
        let entries: [String: ExtensionSnapshotEntry]
    }

    final class ClientSession: @unchecked Sendable {
        static let droppedNotificationDisconnectThreshold = 100
        static let maxConcurrentCommands = 8

        var fd: Int32
        var pendingClose = false
        var commandInFlight = false
        var inFlightCommandCount = 0
        var extensionID: String?
        var subscriptions: Set<String> = []
        var writeBuffer = Data()
        var inputBuffer = Data()
        var writeSource: DispatchSourceWrite?
        var droppedNotificationCount = 0

        init(fd: Int32) {
            self.fd = fd
        }
    }

    private var serverFD: Int32 = -1
    private var acceptSource: DispatchSourceRead?
    private let queue = DispatchQueue(label: "app.muxy.notificationSocket")
    private var subscribers: [ObjectIdentifier: ClientSession] = [:]
    private var liveSessionByExtension: [String: ClientSession] = [:]
    private var readSources: [ObjectIdentifier: DispatchSourceRead] = [:]
    private var extensionSnapshot = ExtensionSnapshot(entries: [:])
    private var inProcessObservers: [UUID: @Sendable (ExtensionEvent) -> Void] = [:]
    private var pendingInvokes: [String: CheckedContinuation<Data, Error>] = [:]
    private var invokeOwner: [String: ObjectIdentifier] = [:]

    var openProjectHandler: (@Sendable (String) -> Void)?
    var installExtensionHandler: (@Sendable (String) -> Void)?
    var commandHandler: (@MainActor @Sendable (String, ClientContext) async -> String)?

    struct ClientContext {
        let extensionID: String?
    }

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

    func applyExtensionSnapshot(_ snapshot: ExtensionSnapshot) {
        queue.async { [weak self] in
            guard let self else { return }
            self.extensionSnapshot = snapshot
            for session in self.subscribers.values {
                guard let extensionID = session.extensionID else { continue }
                guard let entry = snapshot.entries[extensionID] else {
                    session.extensionID = nil
                    session.subscriptions.removeAll()
                    continue
                }
                session.subscriptions = session.subscriptions.filter { event in
                    Self.canSubscribe(entry: entry, to: event)
                }
            }
        }
    }

    private static func canSubscribe(entry: ExtensionSnapshotEntry, to event: String) -> Bool {
        entry.allowedEvents.contains(event) || entry.commandEvents.contains(event)
    }

    static func canSubscribeForTesting(entry: ExtensionSnapshotEntry, to event: String) -> Bool {
        canSubscribe(entry: entry, to: event)
    }

    func broadcast(event: ExtensionEvent) {
        let line = event.serialize() + "\n"
        queue.async { [weak self] in
            guard let self else { return }
            for session in self.subscribers.values where session.subscriptions.contains(event.name) {
                self.enqueueWrite(session: session, text: line)
            }
            for callback in self.inProcessObservers.values {
                callback(event)
            }
        }
    }

    @discardableResult
    func addInProcessObserver(_ callback: @escaping @Sendable (ExtensionEvent) -> Void) -> UUID {
        let token = UUID()
        queue.async { [weak self] in
            self?.inProcessObservers[token] = callback
        }
        return token
    }

    func removeInProcessObserver(_ token: UUID) {
        queue.async { [weak self] in
            self?.inProcessObservers.removeValue(forKey: token)
        }
    }

    static let invokeTimeout: Duration = .seconds(15)

    func invokeRemote(extensionID: String, action: String, payload: Data) async throws -> Data {
        let callID = UUID().uuidString
        let base64 = payload.base64EncodedString()

        let invokeTask = Task {
            try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Data, Error>) in
                queue.async { [weak self] in
                    guard let self else {
                        continuation.resume(throwing: MuxyError.extensionUnavailable)
                        return
                    }
                    guard let session = self.session(forExtension: extensionID) else {
                        continuation.resume(throwing: MuxyError.extensionUnavailable)
                        return
                    }
                    self.pendingInvokes[callID] = continuation
                    self.invokeOwner[callID] = ObjectIdentifier(session)
                    self.enqueueWrite(session: session, text: "invoke|\(callID)|\(action)|\(base64)\n")
                }
            }
        }

        let timeoutTask = Task {
            try? await Task.sleep(for: Self.invokeTimeout)
            guard !Task.isCancelled else { return }
            self.queue.async { [weak self] in
                guard let self, let continuation = self.pendingInvokes.removeValue(forKey: callID) else { return }
                self.invokeOwner.removeValue(forKey: callID)
                continuation.resume(throwing: MuxyError.timeout)
            }
        }

        defer { timeoutTask.cancel() }
        return try await invokeTask.value
    }

    private func session(forExtension extensionID: String) -> ClientSession? {
        liveSessionByExtension[extensionID]
    }

    struct InvokeResult: Equatable {
        let callID: String
        let ok: Bool
        let body: Data
    }

    static func parseInvokeResult(_ message: String) -> InvokeResult? {
        let parts = message.split(separator: "|", maxSplits: 3, omittingEmptySubsequences: false).map(String.init)
        guard parts.count >= 3, parts[0] == "invoke-result", !parts[1].isEmpty else { return nil }
        let status = parts[2]
        guard status == "ok" || status == "err" else { return nil }
        let body = parts.count >= 4 ? (Data(base64Encoded: parts[3]) ?? Data()) : Data()
        return InvokeResult(callID: parts[1], ok: status == "ok", body: body)
    }

    private func handleInvokeResult(_ message: String, session: ClientSession) {
        guard let parsed = Self.parseInvokeResult(message) else { return }
        guard let owner = invokeOwner[parsed.callID], owner == ObjectIdentifier(session) else { return }
        guard let continuation = pendingInvokes.removeValue(forKey: parsed.callID) else { return }
        invokeOwner.removeValue(forKey: parsed.callID)

        if parsed.ok {
            continuation.resume(returning: parsed.body)
            return
        }
        let messageText = String(data: parsed.body, encoding: .utf8) ?? "extension error"
        continuation.resume(throwing: MuxyError.extensionError(messageText))
    }

    private func failPendingInvokes(for session: ClientSession) {
        let owner = ObjectIdentifier(session)
        let callIDs = invokeOwner.filter { $0.value == owner }.map(\.key)
        for callID in callIDs {
            invokeOwner.removeValue(forKey: callID)
            guard let continuation = pendingInvokes.removeValue(forKey: callID) else { continue }
            continuation.resume(throwing: MuxyError.extensionUnavailable)
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

        let flags = fcntl(clientFD, F_GETFL, 0)
        _ = fcntl(clientFD, F_SETFL, flags | O_NONBLOCK)

        let session = ClientSession(fd: clientFD)
        queue.async { [weak self] in
            self?.openSession(session)
        }
    }

    private static let maxMessageSize = 65536

    private static let stickyCommandNames: Set<String> = [
        "subscribe", "identify",
    ]

    private static let commandNames: Set<String> = MuxyAPI.Permissions.verbNames

    private func openSession(_ session: ClientSession) {
        let readSource = DispatchSource.makeReadSource(fileDescriptor: session.fd, queue: queue)
        readSource.setEventHandler { [weak self] in
            self?.readFromSession(session)
        }
        readSource.setCancelHandler { [weak self] in
            self?.closeSession(session)
        }
        readSources[ObjectIdentifier(session)] = readSource
        subscribers[ObjectIdentifier(session)] = session
        readSource.resume()
    }

    private func readFromSession(_ session: ClientSession) {
        var buffer = [UInt8](repeating: 0, count: 4096)
        var reachedEOF = false
        while true {
            let bytesRead = read(session.fd, &buffer, buffer.count)
            if bytesRead > 0 {
                session.inputBuffer.append(contentsOf: buffer[0 ..< bytesRead])
                if session.inputBuffer.count > Self.maxMessageSize {
                    logger.warning("Client exceeded max message size, dropping")
                    disposeSession(session)
                    return
                }
                processBufferedLines(session: session)
                continue
            }
            if bytesRead == 0 {
                reachedEOF = true
                break
            }
            if errno == EAGAIN || errno == EWOULDBLOCK {
                break
            }
            disposeSession(session)
            return
        }

        processBufferedLines(session: session)

        if reachedEOF {
            session.pendingClose = true
            let id = ObjectIdentifier(session)
            if let source = readSources.removeValue(forKey: id) {
                source.cancel()
            }
            subscribers.removeValue(forKey: id)
            if session.writeBuffer.isEmpty, !session.commandInFlight, session.fd >= 0 {
                close(session.fd)
                session.fd = -1
                session.pendingClose = false
            }
        }
    }

    private func processBufferedLines(session: ClientSession) {
        while let newlineRange = session.inputBuffer.range(of: Data([UInt8(ascii: "\n")])) {
            let lineData = session.inputBuffer.subdata(in: 0 ..< newlineRange.lowerBound)
            session.inputBuffer.removeSubrange(0 ..< newlineRange.upperBound)
            handleLine(lineData, session: session)
        }
    }

    private func handleLine(_ data: Data, session: ClientSession) {
        guard !data.isEmpty, let message = String(data: data, encoding: .utf8) else { return }
        let trimmed = message.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }

        let head = trimmed.split(separator: "|", maxSplits: 1, omittingEmptySubsequences: false).first.map(String.init) ?? ""

        if Self.stickyCommandNames.contains(head) {
            let response = evaluateSticky(head: head, message: trimmed, session: session)
            enqueueWrite(session: session, text: response + "\n")
            return
        }

        if Self.commandNames.contains(head) {
            processCommand(trimmed, session: session)
            return
        }

        if head == "invoke-result" {
            handleInvokeResult(trimmed, session: session)
            return
        }

        processNotificationMessage(data, session: session)
    }

    private func evaluateSticky(head: String, message: String, session: ClientSession) -> String {
        switch head {
        case "identify":
            let parts = message.split(separator: "|", maxSplits: 2, omittingEmptySubsequences: false).map(String.init)
            guard parts.count >= 2, !parts[1].isEmpty else { return "error:usage identify|<extension-id>|<token>" }
            let claimedID = parts[1]
            guard let entry = extensionSnapshot.entries[claimedID] else {
                return "error:unknown extension \(claimedID)"
            }
            let providedToken = parts.count >= 3 ? parts[2] : ""
            guard !entry.token.isEmpty, providedToken == entry.token else {
                return "error:invalid extension token"
            }
            session.extensionID = claimedID
            liveSessionByExtension[claimedID] = session
            return "ok"
        case "subscribe":
            let parts = message.split(separator: "|", maxSplits: 1, omittingEmptySubsequences: false).map(String.init)
            guard parts.count == 2, !parts[1].isEmpty else { return "error:usage subscribe|<event>" }
            let event = parts[1]
            if let extensionID = session.extensionID {
                guard let entry = extensionSnapshot.entries[extensionID] else {
                    return "error:extension \(extensionID) is no longer loaded"
                }
                guard Self.canSubscribe(entry: entry, to: event) else {
                    return "error:event \(event) not declared in manifest"
                }
            }
            session.subscriptions.insert(event)
            return "ok"
        default:
            return "error:unknown sticky command \(head)"
        }
    }

    private func processCommand(_ message: String, session: ClientSession) {
        guard let handler = commandHandler else {
            enqueueWrite(session: session, text: "error:no handler registered\n")
            return
        }
        guard session.inFlightCommandCount < ClientSession.maxConcurrentCommands else {
            enqueueWrite(session: session, text: "error:too many concurrent commands\n")
            return
        }
        let context = ClientContext(extensionID: session.extensionID)
        session.commandInFlight = true
        session.inFlightCommandCount += 1
        Task { @Sendable [weak self] in
            let response = await handler(message, context)
            guard let self else { return }
            self.queue.async { [weak self] in
                self?.enqueueWrite(session: session, text: response + "\n")
                session.inFlightCommandCount -= 1
                session.commandInFlight = session.inFlightCommandCount > 0
            }
        }
    }

    private func enqueueWrite(session: ClientSession, text: String) {
        session.writeBuffer.append(contentsOf: Data(text.utf8))
        flushWrites(session: session)
    }

    private func flushWrites(session: ClientSession) {
        while !session.writeBuffer.isEmpty {
            let written = session.writeBuffer.withUnsafeBytes { buffer -> Int in
                guard let ptr = buffer.baseAddress else { return -1 }
                return Darwin.write(session.fd, ptr, buffer.count)
            }
            if written > 0 {
                session.writeBuffer.removeSubrange(0 ..< written)
                continue
            }
            if written < 0, errno == EAGAIN || errno == EWOULDBLOCK {
                scheduleWriteSource(session: session)
                return
            }
            session.pendingClose = false
            disposeSession(session)
            return
        }
        if session.pendingClose {
            if session.fd >= 0 {
                close(session.fd)
                session.fd = -1
            }
            session.pendingClose = false
        }
    }

    private func scheduleWriteSource(session: ClientSession) {
        guard session.writeSource == nil else { return }
        let source = DispatchSource.makeWriteSource(fileDescriptor: session.fd, queue: queue)
        source.setEventHandler { [weak self, weak session] in
            guard let self, let session else { return }
            session.writeSource?.cancel()
            session.writeSource = nil
            self.flushWrites(session: session)
        }
        session.writeSource = source
        source.resume()
    }

    private func processNotificationMessage(_ data: Data, session: ClientSession) {
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

        let installPrefix = "install-extension|"
        if message.hasPrefix(installPrefix) {
            let name = String(message.dropFirst(installPrefix.count))
            guard !name.isEmpty else {
                logger.warning("Ignoring install-extension for empty name")
                return
            }
            logger.info("Received install-extension request via socket")
            installExtensionHandler?(name)
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

        if let extensionID = session.extensionID {
            let entry = extensionSnapshot.entries[extensionID]
            guard entry?.permissions.contains(.notificationsWrite) == true else {
                logger.warning("Dropping notification from \(extensionID): missing notifications:write permission")
                session.droppedNotificationCount += 1
                if session.droppedNotificationCount >= ClientSession.droppedNotificationDisconnectThreshold {
                    logger.warning("Disconnecting \(extensionID) after \(session.droppedNotificationCount) dropped notifications")
                    disposeSession(session)
                }
                return
            }
        }

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

    private func disposeSession(_ session: ClientSession) {
        let id = ObjectIdentifier(session)
        if let source = readSources.removeValue(forKey: id) {
            source.cancel()
        } else {
            closeSession(session)
        }
        subscribers.removeValue(forKey: id)
    }

    private func closeSession(_ session: ClientSession) {
        if let extensionID = session.extensionID, liveSessionByExtension[extensionID] === session {
            liveSessionByExtension.removeValue(forKey: extensionID)
        }
        failPendingInvokes(for: session)
        session.writeSource?.cancel()
        session.writeSource = nil
        if session.fd >= 0, !session.pendingClose {
            close(session.fd)
        }
    }

    private func cleanup() {
        acceptSource?.cancel()
        acceptSource = nil
        for session in Array(subscribers.values) {
            disposeSession(session)
        }
    }
}
