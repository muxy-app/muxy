import Darwin
import Foundation
import Testing

@testable import Muxy

@Suite("NotificationSocketServer connection lifecycle")
struct NotificationSocketAcceptLoopTests {
    @Test("serves many sequential CLI connections without wedging the listener")
    func servesSequentialConnections() async throws {
        let path = Self.temporarySocketPath()
        let server = NotificationSocketServer(socketPath: path)
        server.commandHandler = { _, _ in "list-tabs|ok" }
        server.start()
        await server.awaitReady()
        defer { server.stop() }

        for index in 0 ..< 40 {
            let reply = try Self.sendCommand("list-tabs", to: path)
            #expect(reply == "list-tabs|ok", "connection \(index) failed")
        }
    }

    @Test("closes the connection after a CLI reply so the client returns immediately")
    func closesAfterReply() async throws {
        let path = Self.temporarySocketPath()
        let server = NotificationSocketServer(socketPath: path)
        server.commandHandler = { _, _ in "list-tabs|ok" }
        server.start()
        await server.awaitReady()
        defer { server.stop() }

        let fd = try Self.connect(to: path)
        defer { close(fd) }
        try Self.writeLine("list-tabs", to: fd)
        shutdown(fd, SHUT_WR)

        let payload = try Self.readUntilEOF(fd)
        #expect(payload.last == NotificationSocketServer.commandReplyTerminator)
        #expect(String(decoding: payload.dropLast(), as: UTF8.self) == "list-tabs|ok")
    }

    private static func temporarySocketPath() -> String {
        let directory = FileManager.default.temporaryDirectory
        return directory.appendingPathComponent("muxy-test-\(UUID().uuidString).sock").path
    }

    private static func sendCommand(_ command: String, to path: String) throws -> String {
        let fd = try connect(to: path)
        defer { close(fd) }
        try writeLine(command, to: fd)
        shutdown(fd, SHUT_WR)
        let payload = try readCommandReply(fd)
        return String(decoding: payload, as: UTF8.self)
    }

    private static func readCommandReply(_ fd: Int32, deadline: TimeInterval = 5) throws -> Data {
        var collected = Data()
        var buffer = [UInt8](repeating: 0, count: 4096)
        let end = Date().addingTimeInterval(deadline)
        while Date() < end {
            try waitForRead(fd, timeout: end.timeIntervalSinceNow)
            let count = Darwin.read(fd, &buffer, buffer.count)
            if count > 0 {
                if let terminator = buffer[0 ..< count].firstIndex(of: NotificationSocketServer.commandReplyTerminator) {
                    collected.append(contentsOf: buffer[0 ..< terminator])
                    return collected
                }
                collected.append(contentsOf: buffer[0 ..< count])
                continue
            }
            if count == 0 { throw SocketError.missingReplyTerminator }
            if errno == EINTR { continue }
            throw SocketError.readFailed(errno)
        }
        throw SocketError.timedOut
    }

    private static func connect(to path: String) throws -> Int32 {
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        try #require(fd >= 0)
        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        _ = withUnsafeMutablePointer(to: &addr.sun_path) { pointer in
            pointer.withMemoryRebound(to: CChar.self, capacity: 104) { destination in
                path.withCString { strncpy(destination, $0, 103) }
            }
        }
        let result = withUnsafePointer(to: &addr) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard result == 0 else {
            close(fd)
            throw SocketError.connectFailed(errno)
        }
        return fd
    }

    private static func writeLine(_ line: String, to fd: Int32) throws {
        let data = Data((line + "\n").utf8)
        try data.withUnsafeBytes { buffer in
            var sent = 0
            while sent < buffer.count {
                let written = Darwin.write(fd, buffer.baseAddress!.advanced(by: sent), buffer.count - sent)
                guard written > 0 else { throw SocketError.writeFailed(errno) }
                sent += written
            }
        }
    }

    private static func readUntilEOF(_ fd: Int32, deadline: TimeInterval = 5) throws -> Data {
        var collected = Data()
        var buffer = [UInt8](repeating: 0, count: 4096)
        let end = Date().addingTimeInterval(deadline)
        while Date() < end {
            try waitForRead(fd, timeout: end.timeIntervalSinceNow)
            let count = Darwin.read(fd, &buffer, buffer.count)
            if count > 0 {
                collected.append(contentsOf: buffer[0 ..< count])
                continue
            }
            if count == 0 { return collected }
            if errno == EINTR || errno == EAGAIN || errno == EWOULDBLOCK {
                usleep(2000)
                continue
            }
            throw SocketError.readFailed(errno)
        }
        throw SocketError.timedOut
    }

    private static func waitForRead(_ fd: Int32, timeout: TimeInterval) throws {
        var event = pollfd(fd: fd, events: Int16(POLLIN), revents: 0)
        let milliseconds = max(1, Int32(min(timeout * 1000, Double(Int32.max))))
        while true {
            let ready = poll(&event, 1, milliseconds)
            if ready > 0 { return }
            if ready == 0 { throw SocketError.timedOut }
            if errno == EINTR { continue }
            throw SocketError.readFailed(errno)
        }
    }

    private enum SocketError: Error {
        case connectFailed(Int32)
        case writeFailed(Int32)
        case readFailed(Int32)
        case missingReplyTerminator
        case timedOut
    }
}
