import Darwin
import Dispatch
import Foundation
import MuxyShared

public enum AgentHookSocketError: Error, Equatable {
    case invalidSocketPath
    case socketOperation(String)
    case deliveryTimedOut
    case connectionClosed
    case invalidAcknowledgement
    case noAttempts
}

public struct AgentHookSocketClient {
    public typealias SendAttempt = (String, Data, TimeInterval) throws -> Void

    public static let defaultMaximumAttempts = 3
    public static let defaultTotalBudget: TimeInterval = 0.4
    public static let defaultRetryDelay: TimeInterval = 0.02

    private let maximumAttempts: Int
    private let totalBudget: TimeInterval
    private let retryDelay: TimeInterval
    private let sendAttempt: SendAttempt
    private let sleep: (TimeInterval) -> Void
    private let elapsed: () -> TimeInterval

    public init(
        maximumAttempts: Int = defaultMaximumAttempts,
        totalBudget: TimeInterval = defaultTotalBudget,
        retryDelay: TimeInterval = defaultRetryDelay,
        sendAttempt: @escaping SendAttempt = sendOnce,
        sleep: @escaping (TimeInterval) -> Void = Thread.sleep,
        elapsed: @escaping () -> TimeInterval = AgentHookSocketClient.processUptime
    ) {
        self.maximumAttempts = maximumAttempts
        self.totalBudget = totalBudget
        self.retryDelay = retryDelay
        self.sendAttempt = sendAttempt
        self.sleep = sleep
        self.elapsed = elapsed
    }

    public static func processUptime() -> TimeInterval {
        TimeInterval(DispatchTime.now().uptimeNanoseconds) / 1_000_000_000
    }

    public func send(_ message: AgentHookEventMessage, to socketPath: String) throws {
        try send(
            message,
            to: socketPath,
            availableBudget: totalBudget
        )
    }

    public func send(
        _ message: AgentHookEventMessage,
        to socketPath: String,
        budget: AgentHookExecutionBudget
    ) throws {
        try send(
            message,
            to: socketPath,
            availableBudget: min(totalBudget, budget.remainingDuration)
        )
    }

    private func send(
        _ message: AgentHookEventMessage,
        to socketPath: String,
        availableBudget: TimeInterval
    ) throws {
        guard maximumAttempts > 0 else { throw AgentHookSocketError.noAttempts }
        let line = try AgentHookWireCodec.encodeEventLine(message)
        let start = elapsed()
        var lastError: (any Error)?

        for attempt in 0 ..< maximumAttempts {
            let remaining = availableBudget - (elapsed() - start)
            guard remaining > 0 else { break }
            do {
                try sendAttempt(socketPath, line, remaining)
                return
            } catch {
                lastError = error
                guard attempt + 1 < maximumAttempts else { break }
                let afterAttempt = availableBudget - (elapsed() - start)
                guard afterAttempt > retryDelay else { break }
                sleep(retryDelay)
            }
        }

        throw lastError ?? AgentHookSocketError.deliveryTimedOut
    }

    public static func sendOnce(socketPath: String, line: Data, remainingBudget: TimeInterval) throws {
        let descriptor = socket(AF_UNIX, SOCK_STREAM, 0)
        guard descriptor >= 0 else { throw socketError() }
        defer { close(descriptor) }

        try configure(descriptor: descriptor)
        let deadline = AgentHookExecutionBudget(duration: remainingBudget)
        try connect(descriptor: descriptor, socketPath: socketPath, deadline: deadline)
        try exchange(line: line, descriptor: descriptor, deadline: deadline)
    }

    public static func sendConnected(
        descriptor: Int32,
        line: Data,
        remainingBudget: TimeInterval
    ) throws {
        try configure(descriptor: descriptor)
        try exchange(
            line: line,
            descriptor: descriptor,
            deadline: AgentHookExecutionBudget(duration: remainingBudget)
        )
    }

    private static func configure(descriptor: Int32) throws {
        var suppressBrokenPipe: Int32 = 1
        guard setsockopt(
            descriptor,
            SOL_SOCKET,
            SO_NOSIGPIPE,
            &suppressBrokenPipe,
            socklen_t(MemoryLayout<Int32>.size)
        ) == 0
        else { throw socketError() }

        let flags = fcntl(descriptor, F_GETFL, 0)
        guard flags >= 0, fcntl(descriptor, F_SETFL, flags | O_NONBLOCK) == 0 else {
            throw socketError()
        }
    }

    private static func exchange(line: Data, descriptor: Int32, deadline: AgentHookExecutionBudget) throws {
        try write(line, to: descriptor, deadline: deadline)
        let acknowledgementData = try readLine(from: descriptor, deadline: deadline)
        let acknowledgement = try AgentHookWireCodec.decodeAcknowledgementLine(acknowledgementData)
        guard acknowledgement.v == AgentHookProtocol.version,
              acknowledgement.kind == AgentHookProtocol.acknowledgementKind,
              acknowledgement.ok
        else { throw AgentHookSocketError.invalidAcknowledgement }
    }

    private static func connect(
        descriptor: Int32,
        socketPath: String,
        deadline: AgentHookExecutionBudget
    ) throws {
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        let capacity = MemoryLayout.size(ofValue: address.sun_path)
        guard !socketPath.isEmpty, socketPath.utf8.count < capacity else {
            throw AgentHookSocketError.invalidSocketPath
        }

        withUnsafeMutablePointer(to: &address.sun_path) { pointer in
            let destination = pointer.withMemoryRebound(to: CChar.self, capacity: capacity) { $0 }
            _ = socketPath.withCString { source in
                strncpy(destination, source, capacity - 1)
            }
        }

        let result = withUnsafePointer(to: &address) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.connect(descriptor, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        if result == 0 {
            return
        }

        let code = errno
        guard code == EINPROGRESS || code == EALREADY || code == EAGAIN || code == EWOULDBLOCK else {
            throw socketError(code)
        }

        try waitForReadiness(descriptor: descriptor, events: Int16(POLLOUT), deadline: deadline)
        var connectionError: Int32 = 0
        var connectionErrorSize = socklen_t(MemoryLayout<Int32>.size)
        guard getsockopt(
            descriptor,
            SOL_SOCKET,
            SO_ERROR,
            &connectionError,
            &connectionErrorSize
        ) == 0
        else { throw socketError() }
        guard connectionError == 0 else { throw socketError(connectionError) }
    }

    private static func write(
        _ data: Data,
        to descriptor: Int32,
        deadline: AgentHookExecutionBudget
    ) throws {
        try data.withUnsafeBytes { rawBuffer in
            guard let baseAddress = rawBuffer.baseAddress else { return }
            var offset = 0
            while offset < rawBuffer.count {
                let written = Darwin.write(
                    descriptor,
                    baseAddress.advanced(by: offset),
                    rawBuffer.count - offset
                )
                if written > 0 {
                    offset += written
                    continue
                }
                if written < 0, errno == EINTR {
                    continue
                }
                if written < 0, errno == EAGAIN || errno == EWOULDBLOCK {
                    try waitForReadiness(
                        descriptor: descriptor,
                        events: Int16(POLLOUT),
                        deadline: deadline
                    )
                    continue
                }
                throw socketError()
            }
        }
    }

    private static func readLine(from descriptor: Int32, deadline: AgentHookExecutionBudget) throws -> Data {
        var collected = Data()
        var buffer = [UInt8](repeating: 0, count: 512)

        while collected.count <= 4096 {
            try waitForReadiness(descriptor: descriptor, events: Int16(POLLIN), deadline: deadline)

            let count = Darwin.read(descriptor, &buffer, buffer.count)
            if count > 0 {
                collected.append(contentsOf: buffer[0 ..< count])
                if let newline = collected.firstIndex(of: UInt8(ascii: "\n")) {
                    return collected.prefix(through: newline)
                }
                continue
            }
            if count < 0, errno == EINTR {
                continue
            }
            if count < 0, errno == EAGAIN || errno == EWOULDBLOCK {
                continue
            }
            if count < 0 {
                throw socketError()
            }
            throw AgentHookSocketError.connectionClosed
        }

        throw AgentHookSocketError.invalidAcknowledgement
    }

    private static func waitForReadiness(
        descriptor: Int32,
        events: Int16,
        deadline: AgentHookExecutionBudget
    ) throws {
        while true {
            guard let timeout = deadline.pollTimeoutMilliseconds else {
                throw AgentHookSocketError.deliveryTimedOut
            }
            var event = pollfd(fd: descriptor, events: events, revents: 0)
            let ready = poll(&event, 1, timeout)
            if ready > 0 {
                return
            }
            if ready == 0 {
                throw AgentHookSocketError.deliveryTimedOut
            }
            if errno == EINTR {
                continue
            }
            throw socketError()
        }
    }

    private static func socketError(_ code: Int32 = errno) -> AgentHookSocketError {
        AgentHookSocketError.socketOperation(String(cString: strerror(code)))
    }
}
