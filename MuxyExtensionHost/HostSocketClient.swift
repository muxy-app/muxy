import Darwin
import Foundation
import MuxyShared

final class HostSocketClient: @unchecked Sendable {
    enum ClientError: Error {
        case connectFailed(String)
        case notConnected
        case closed
    }

    private let fd: Int32
    private let writeLock = NSLock()
    private let replyLock = NSCondition()
    private var pendingReply: String?
    private var hasReply = false
    private var closed = false
    private var readBuffer = Data()
    private var eventHandler: ((String) -> Void)?
    private var extensionEventHandler: ((String) -> Void)?
    private var invokeHandler: ((String) -> Void)?
    private var modalResultHandler: ((String) -> Void)?

    static let maxConnectAttempts = 15
    static let connectRetryDelay: TimeInterval = 0.1
    static let maxIdentifyAttempts = 15
    static let identifyRetryDelay: TimeInterval = 0.1

    static func isTransientIdentifyRejection(_ reply: String) -> Bool {
        reply.hasPrefix("error:unknown extension")
    }

    init(
        socketPath: String,
        maxConnectAttempts: Int = HostSocketClient.maxConnectAttempts,
        connectRetryDelay: TimeInterval = HostSocketClient.connectRetryDelay
    ) throws {
        var lastError = ""
        for attempt in 1 ... maxConnectAttempts {
            do {
                fd = try Self.connect(to: socketPath)
                return
            } catch let ClientError.connectFailed(reason) {
                lastError = reason
                guard attempt < maxConnectAttempts else { break }
                Thread.sleep(forTimeInterval: connectRetryDelay)
            }
        }
        throw ClientError.connectFailed(lastError)
    }

    private static func connect(to socketPath: String) throws -> Int32 {
        let descriptor = socket(AF_UNIX, SOCK_STREAM, 0)
        guard descriptor >= 0 else {
            throw ClientError.connectFailed(String(cString: strerror(errno)))
        }

        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        withUnsafeMutablePointer(to: &addr.sun_path) { ptr in
            let bound = ptr.withMemoryRebound(to: CChar.self, capacity: 104) { $0 }
            _ = socketPath.withCString { strncpy(bound, $0, 103) }
        }

        let result = withUnsafePointer(to: &addr) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.connect(descriptor, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard result == 0 else {
            close(descriptor)
            throw ClientError.connectFailed(String(cString: strerror(errno)))
        }

        return descriptor
    }

    init(fileDescriptor: Int32) {
        fd = fileDescriptor
    }

    func onEvent(_ handler: @escaping (String) -> Void) {
        eventHandler = handler
    }

    func onExtensionEvent(_ handler: @escaping (String) -> Void) {
        extensionEventHandler = handler
    }

    func onInvoke(_ handler: @escaping (String) -> Void) {
        invokeHandler = handler
    }

    func onModalResult(_ handler: @escaping (String) -> Void) {
        modalResultHandler = handler
    }

    func startReading() {
        Thread.detachNewThread { [weak self] in
            self?.readLoop()
        }
    }

    func send(_ line: String) throws {
        writeLock.lock()
        defer { writeLock.unlock() }
        guard !closed else { throw ClientError.closed }
        let data = Data((line + "\n").utf8)
        try data.withUnsafeBytes { raw in
            var offset = 0
            let base = raw.bindMemory(to: UInt8.self).baseAddress!
            while offset < data.count {
                let written = Darwin.write(fd, base + offset, data.count - offset)
                if written > 0 {
                    offset += written
                    continue
                }
                if written < 0, errno == EINTR { continue }
                throw ClientError.closed
            }
        }
    }

    func sendAndWaitReply(_ line: String) throws -> String {
        replyLock.lock()
        hasReply = false
        pendingReply = nil
        replyLock.unlock()

        try send(line)

        replyLock.lock()
        defer { replyLock.unlock() }
        while !hasReply {
            if closed { throw ClientError.closed }
            replyLock.wait()
        }
        guard let reply = pendingReply else { throw ClientError.closed }
        return reply
    }

    private func readLoop() {
        var chunk = [UInt8](repeating: 0, count: 4096)
        while true {
            let bytesRead = read(fd, &chunk, chunk.count)
            if bytesRead > 0 {
                readBuffer.append(contentsOf: chunk[0 ..< bytesRead])
                drainLines()
                continue
            }
            if bytesRead < 0, errno == EINTR { continue }
            break
        }
        markClosed()
    }

    private func drainLines() {
        while let range = readBuffer.range(of: Data([UInt8(ascii: "\n")])) {
            let lineData = readBuffer.subdata(in: 0 ..< range.lowerBound)
            readBuffer.removeSubrange(0 ..< range.upperBound)
            guard let line = String(data: lineData, encoding: .utf8) else { continue }
            deliver(line)
        }
    }

    private func deliver(_ line: String) {
        if line.hasPrefix("event|") {
            eventHandler?(line)
            return
        }
        if line.hasPrefix("\(ExtensionLocalEvent.messageHead)|") {
            extensionEventHandler?(line)
            return
        }
        if line.hasPrefix("invoke|") {
            invokeHandler?(line)
            return
        }
        if line.hasPrefix("\(ExtensionModalResult.messageHead)|") {
            modalResultHandler?(line)
            return
        }
        replyLock.lock()
        pendingReply = line
        hasReply = true
        replyLock.signal()
        replyLock.unlock()
    }

    private func markClosed() {
        replyLock.lock()
        closed = true
        replyLock.signal()
        replyLock.unlock()
    }

    var isClosed: Bool {
        replyLock.lock()
        defer { replyLock.unlock() }
        return closed
    }
}
