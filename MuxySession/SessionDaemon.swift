import Darwin
import MuxySessionProtocol

final class SessionDaemon {
    static let replayCapacity = 256 * 1024
    static let clientOutputLimit = 4 * 1024 * 1024
    static let sessionInputLimit = 1024 * 1024
    static let maximumConnections = 64
    static let idleTimeoutMilliseconds: Int32 = 10000
    static let replayChunkSize = 32 * 1024

    private let socketPath: String
    private let idleTimeout: Int32
    private let listener: Int32
    private let lockDescriptor: Int32
    private let signalPipe: SessionSignalPipe

    private var sessions: [SessionIdentifier: PTYSession] = [:]
    private var sessionsByMaster: [Int32: SessionIdentifier] = [:]
    private var connections: [Int32: SessionConnection] = [:]
    private var closingDescriptors: [Int32] = []

    init?(socketPath: String, idleTimeoutMilliseconds: Int32 = SessionDaemon.idleTimeoutMilliseconds) {
        self.socketPath = socketPath
        idleTimeout = idleTimeoutMilliseconds
        signal(SIGPIPE, SIG_IGN)
        signal(SIGHUP, SIG_IGN)

        guard let lock = SessionSocket.acquireLock(path: socketPath + ".lock") else {
            return nil
        }
        lockDescriptor = lock

        guard let signalPipe = SessionSignalPipe(signals: [SIGCHLD]) else {
            SessionIO.close(lock)
            return nil
        }
        self.signalPipe = signalPipe

        guard let listener = SessionSocket.makeListener(path: socketPath) else {
            SessionIO.close(lock)
            return nil
        }
        self.listener = listener
    }

    func run() {
        while true {
            let isIdle = sessions.isEmpty && connections.isEmpty
            var descriptors = buildPollDescriptors()
            let ready = poll(&descriptors, nfds_t(descriptors.count), isIdle ? idleTimeout : -1)
            if ready == 0 {
                guard sessions.isEmpty, connections.isEmpty else { continue }
                acceptConnections()
                guard connections.isEmpty else { continue }
                break
            }
            if ready < 0 {
                guard errno == EINTR else { break }
                continue
            }
            handle(descriptors)
            reapChildren()
            flushConnections()
            closeMarkedConnections()
        }
        shutdown()
    }

    private func shutdown() {
        for session in Array(sessions.values) {
            closeMaster(session)
        }
        for descriptor in connections.keys {
            SessionIO.close(descriptor)
        }
        SessionIO.close(listener)
        unlink(socketPath)
        SessionIO.close(lockDescriptor)
        unlink(socketPath + ".lock")
    }

    private func buildPollDescriptors() -> [pollfd] {
        var descriptors: [pollfd] = [
            pollfd(fd: listener, events: Int16(POLLIN), revents: 0),
            pollfd(fd: signalPipe.readDescriptor, events: Int16(POLLIN), revents: 0),
        ]
        for connection in connections.values {
            var events = Int32(POLLIN)
            if connection.hasPendingOutput {
                events |= Int32(POLLOUT)
            }
            descriptors.append(pollfd(fd: connection.descriptor, events: Int16(events), revents: 0))
        }
        for session in sessions.values where session.masterDescriptor >= 0 {
            var events: Int32 = 0
            if !isClientBackedUp(session) {
                events |= Int32(POLLIN)
            }
            if session.hasPendingInput {
                events |= Int32(POLLOUT)
            }
            guard events != 0 else { continue }
            descriptors.append(pollfd(fd: session.masterDescriptor, events: Int16(events), revents: 0))
        }
        return descriptors
    }

    private func isClientBackedUp(_ session: PTYSession) -> Bool {
        guard let descriptor = session.clientDescriptor, let connection = connections[descriptor] else { return false }
        return connection.pendingByteCount >= Self.clientOutputLimit
    }

    private func handle(_ descriptors: [pollfd]) {
        for entry in descriptors where entry.revents != 0 {
            if entry.fd == listener {
                acceptConnections()
            } else if entry.fd == signalPipe.readDescriptor {
                signalPipe.drain()
            } else if connections[entry.fd] != nil {
                handleConnection(entry)
            } else if let identifier = sessionsByMaster[entry.fd] {
                handleMaster(entry, identifier: identifier)
            }
        }
    }

    private func acceptConnections() {
        while let descriptor = SessionSocket.accept(listener) {
            guard connections.count < Self.maximumConnections else {
                SessionIO.close(descriptor)
                continue
            }
            guard SessionSocket.peerUserID(descriptor) == getuid() else {
                SessionLog.write("rejected connection from another user")
                SessionIO.close(descriptor)
                continue
            }
            connections[descriptor] = SessionConnection(descriptor: descriptor)
        }
    }

    private func handleConnection(_ entry: pollfd) {
        guard let connection = connections[entry.fd] else { return }
        if entry.revents & Int16(POLLOUT) != 0, !connection.flush() {
            markForClose(entry.fd)
            return
        }
        guard entry.revents & Int16(POLLIN) != 0 || entry.revents & Int16(POLLHUP) != 0 else { return }

        var reachedEnd = false
        readLoop: while true {
            switch SessionIO.read(entry.fd) {
            case let .bytes(bytes):
                connection.decoder.push(bytes)
            case .wouldBlock:
                break readLoop
            case .endOfFile,
                 .failed:
                reachedEnd = true
                break readLoop
            }
        }

        while true {
            do {
                guard let frame = try connection.decoder.next() else { break }
                handle(frame: frame, connection: connection)
            } catch {
                SessionLog.write("dropping connection with a malformed frame")
                markForClose(entry.fd)
                return
            }
        }

        if reachedEnd {
            markForClose(entry.fd)
        }
    }

    private func handle(frame: SessionFrame, connection: SessionConnection) {
        switch frame.kind {
        case .attach:
            handleAttach(payload: frame.payload, connection: connection)
        case .input:
            guard let identifier = connection.attachedSession, let session = sessions[identifier] else { return }
            session.appendInput(frame.payload, limit: Self.sessionInputLimit)
            session.flushInput()
        case .resize:
            guard let identifier = connection.attachedSession,
                  let session = sessions[identifier],
                  let size = try? SessionResizePayload.decode(frame.payload)
            else { return }
            guard SessionWindowSizePolicy.isUsable(columns: size.columns, rows: size.rows) else { return }
            SessionPTY.resize(masterDescriptor: session.masterDescriptor, columns: size.columns, rows: size.rows)
        case .list:
            connection.enqueue(SessionFrame(
                kind: .sessions,
                payload: SessionDescriptor.encodeList(sessions.values.map(\.descriptor))
            ))
        case .info:
            guard let identifier = try? SessionIdentifierPayload.decode(frame.payload) else { return }
            let matches = sessions[identifier].map { [$0.descriptor] } ?? []
            connection.enqueue(SessionFrame(kind: .sessions, payload: SessionDescriptor.encodeList(matches)))
        case .kill:
            let stopped = if let identifier = try? SessionIdentifierPayload.decode(frame.payload),
                             let session = sessions[identifier]
            {
                endSession(session, status: 0, force: true)
            } else {
                true
            }
            enqueueStopResult(stopped, connection: connection)
        case .killAll:
            enqueueStopResult(endSessions(Array(sessions.values), status: 0), connection: connection)
        case .attached,
             .output,
             .exited,
             .failure,
             .sessions,
             .acknowledged:
            break
        }
    }

    private func handleAttach(payload: [UInt8], connection: SessionConnection) {
        guard let request = try? SessionAttachRequest.decode(payload) else {
            connection.enqueue(SessionFrame(kind: .failure, payload: SessionTextPayload.encode("malformed attach request")))
            connection.closesAfterFlush = true
            return
        }
        guard request.version == SessionProtocolVersion.current else {
            connection.enqueue(SessionFrame(
                kind: .failure,
                payload: SessionTextPayload.encode(
                    "this background session is owned by a different version of Muxy; stop it to start a new one"
                )
            ))
            connection.closesAfterFlush = true
            return
        }
        guard connection.attachedSession == nil else { return }

        if let session = sessions[request.identifier] {
            attach(session: session, request: request, connection: connection)
            return
        }
        create(request: request, connection: connection)
    }

    private func attach(session: PTYSession, request: SessionAttachRequest, connection: SessionConnection) {
        if let previous = session.clientDescriptor, previous != connection.descriptor {
            connections[previous]?.attachedSession = nil
            markForClose(previous)
        }
        session.clientDescriptor = connection.descriptor
        session.metadata = request.metadata
        connection.attachedSession = session.identifier

        if SessionWindowSizePolicy.isUsable(columns: request.columns, rows: request.rows) {
            SessionPTY.resize(
                masterDescriptor: session.masterDescriptor,
                columns: request.columns,
                rows: request.rows
            )
        }
        connection.enqueue(SessionFrame(
            kind: .attached,
            payload: SessionAttachAccepted(
                created: false,
                shellProcessID: session.processID,
                ttyDevice: session.ttyDevice
            ).encoded()
        ))
        enqueueReplay(session: session, connection: connection)
        SessionPTY.requestRedraw(masterDescriptor: session.masterDescriptor, fallbackProcessID: session.processID)
    }

    private func enqueueReplay(session: PTYSession, connection: SessionConnection) {
        let bytes = session.replay.replayBytes
        guard !bytes.isEmpty else { return }
        var index = 0
        while index < bytes.count {
            let end = min(index + Self.replayChunkSize, bytes.count)
            connection.enqueue(SessionFrame(kind: .output, payload: Array(bytes[index ..< end])))
            index = end
        }
    }

    private func create(request: SessionAttachRequest, connection: SessionConnection) {
        let invocation = SessionShellIntegration.invocation(
            command: request.command,
            shell: request.shell,
            resourcesDirectory: request.resourcesDirectory,
            environment: request.environment
        )
        let size = SessionWindowSizePolicy.createSize(columns: request.columns, rows: request.rows)
        guard let process = SessionPTY.spawn(
            invocation: invocation,
            workingDirectory: request.workingDirectory,
            columns: size.columns,
            rows: size.rows
        )
        else {
            connection.enqueue(SessionFrame(kind: .failure, payload: SessionTextPayload.encode("failed to start the session")))
            connection.closesAfterFlush = true
            return
        }

        let session = PTYSession(
            identifier: request.identifier,
            process: process,
            workingDirectory: request.workingDirectory,
            replayCapacity: Self.replayCapacity
        )
        session.clientDescriptor = connection.descriptor
        session.metadata = request.metadata
        sessions[request.identifier] = session
        sessionsByMaster[process.masterDescriptor] = request.identifier
        connection.attachedSession = request.identifier

        connection.enqueue(SessionFrame(
            kind: .attached,
            payload: SessionAttachAccepted(
                created: true,
                shellProcessID: process.processID,
                ttyDevice: process.ttyDevice
            ).encoded()
        ))
    }

    private func handleMaster(_ entry: pollfd, identifier: SessionIdentifier) {
        guard let session = sessions[identifier] else { return }
        if entry.revents & Int16(POLLOUT) != 0 {
            session.flushInput()
        }
        guard entry.revents & Int16(POLLIN) != 0
            || entry.revents & Int16(POLLHUP) != 0
            || entry.revents & Int16(POLLNVAL) != 0
        else { return }

        readLoop: while true {
            switch SessionIO.read(session.masterDescriptor) {
            case let .bytes(bytes):
                session.replay.append(bytes)
                if let descriptor = session.clientDescriptor, let connection = connections[descriptor] {
                    connection.enqueue(SessionFrame(kind: .output, payload: bytes))
                }
                if isClientBackedUp(session) {
                    break readLoop
                }
            case .wouldBlock:
                break readLoop
            case .endOfFile,
                 .failed:
                closeMaster(session)
                return
            }
        }
    }

    private func closeMaster(_ session: PTYSession) {
        guard session.masterDescriptor >= 0 else { return }
        sessionsByMaster.removeValue(forKey: session.masterDescriptor)
        SessionIO.close(session.masterDescriptor)
        session.masterDescriptor = -1
        session.isEnding = true
        SessionPTY.terminate(processID: session.processID)
    }

    @discardableResult
    private func endSession(_ session: PTYSession, status: Int32, force: Bool = false) -> Bool {
        closeMaster(session)
        guard !force || SessionPTY.forceTerminate(processID: session.processID) else {
            SessionLog.write("failed to stop session \(session.identifier.uuidString)")
            return false
        }
        finishSession(session, status: status)
        return true
    }

    private func endSessions(_ endingSessions: [PTYSession], status: Int32) -> Bool {
        for session in endingSessions {
            closeMaster(session)
        }
        let failedProcessIDs = SessionPTY.forceTerminate(processIDs: endingSessions.map(\.processID))
        for session in endingSessions where !failedProcessIDs.contains(session.processID) {
            finishSession(session, status: status)
        }
        for session in endingSessions where failedProcessIDs.contains(session.processID) {
            SessionLog.write("failed to stop session \(session.identifier.uuidString)")
        }
        return failedProcessIDs.isEmpty
    }

    private func finishSession(_ session: PTYSession, status: Int32) {
        sessions.removeValue(forKey: session.identifier)
        guard let descriptor = session.clientDescriptor, let connection = connections[descriptor] else { return }
        connection.enqueue(SessionFrame(kind: .exited, payload: SessionExitPayload.encode(status: status)))
        connection.attachedSession = nil
        connection.closesAfterFlush = true
        session.clientDescriptor = nil
    }

    private func enqueueStopResult(_ stopped: Bool, connection: SessionConnection) {
        let frame = stopped
            ? SessionFrame(kind: .acknowledged)
            : SessionFrame(kind: .failure, payload: SessionTextPayload.encode("failed to stop one or more sessions"))
        connection.enqueue(frame)
    }

    private func reapChildren() {
        while true {
            var status: Int32 = 0
            let processID = waitpid(-1, &status, WNOHANG)
            guard processID > 0 else { return }
            guard let session = sessions.values.first(where: { $0.processID == processID }) else { continue }
            endSession(session, status: Self.exitCode(status))
        }
    }

    static func exitCode(_ status: Int32) -> Int32 {
        let terminationSignal = status & 0o177
        if terminationSignal == 0 {
            return (status >> 8) & 0xFF
        }
        if terminationSignal == 0o177 {
            return 0
        }
        return 128 + terminationSignal
    }

    private func flushConnections() {
        for connection in connections.values {
            if !connection.flush() {
                markForClose(connection.descriptor)
                continue
            }
            if connection.closesAfterFlush, !connection.hasPendingOutput {
                markForClose(connection.descriptor)
            }
        }
    }

    private func markForClose(_ descriptor: Int32) {
        guard !closingDescriptors.contains(descriptor) else { return }
        closingDescriptors.append(descriptor)
    }

    private func closeMarkedConnections() {
        let descriptors = closingDescriptors
        closingDescriptors.removeAll(keepingCapacity: true)
        for descriptor in descriptors {
            guard let connection = connections.removeValue(forKey: descriptor) else { continue }
            if let identifier = connection.attachedSession,
               let session = sessions[identifier],
               session.clientDescriptor == descriptor
            {
                session.clientDescriptor = nil
            }
            SessionIO.close(descriptor)
        }
    }
}
