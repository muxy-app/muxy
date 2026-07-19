import Darwin
import Foundation
import MuxyShared
import Testing

@testable import Muxy

@Suite("NotificationSocketServer agent hook acknowledgement")
struct NotificationSocketAgentHookTests {
    @Test("acknowledges a valid protocol v3 event")
    func acknowledgesValidProtocolV3Event() async throws {
        let path = Self.temporarySocketPath()
        let server = NotificationSocketServer(socketPath: path)
        server.start()
        await server.awaitReady()
        defer { server.stop() }

        let descriptor = try Self.connect(to: path)
        defer { close(descriptor) }
        let event = AgentHookEventMessage(
            provider: "claude_hook",
            paneID: UUID().uuidString,
            phase: .working,
            title: "",
            body: "",
            pids: [],
            ts: 1_721_234_567
        )

        try Self.write(AgentHookWireCodec.encodeEventLine(event), to: descriptor)
        let acknowledgement = try AgentHookWireCodec.decodeAcknowledgementLine(
            Self.readLine(from: descriptor)
        )

        #expect(acknowledgement == AgentHookAcknowledgement(ok: true))
    }

    @Test("acknowledges a synthetic test event")
    func acknowledgesTestEvent() async throws {
        let path = Self.temporarySocketPath()
        let server = NotificationSocketServer(socketPath: path)
        server.start()
        await server.awaitReady()
        defer { server.stop() }

        let descriptor = try Self.connect(to: path)
        defer { close(descriptor) }
        let event = AgentHookEventMessage(
            provider: "claude_hook",
            paneID: nil,
            phase: .finished,
            title: "Claude Code test",
            body: "Hook pipeline is working",
            pids: [],
            ts: 1_721_234_567,
            test: true
        )

        try Self.write(AgentHookWireCodec.encodeEventLine(event), to: descriptor)
        let acknowledgement = try AgentHookWireCodec.decodeAcknowledgementLine(
            Self.readLine(from: descriptor)
        )

        #expect(acknowledgement == AgentHookAcknowledgement(ok: true))
    }

    private static func temporarySocketPath() -> String {
        FileManager.default.temporaryDirectory
            .appendingPathComponent("mah-\(UUID().uuidString.prefix(8)).sock")
            .path
    }

    private static func connect(to path: String) throws -> Int32 {
        let descriptor = socket(AF_UNIX, SOCK_STREAM, 0)
        guard descriptor >= 0 else { throw SocketError.createFailed(errno) }
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        let capacity = MemoryLayout.size(ofValue: address.sun_path)
        guard path.utf8.count < capacity else {
            close(descriptor)
            throw SocketError.pathTooLong
        }
        withUnsafeMutablePointer(to: &address.sun_path) { pointer in
            let destination = pointer.withMemoryRebound(to: CChar.self, capacity: capacity) { $0 }
            _ = path.withCString { strncpy(destination, $0, capacity - 1) }
        }
        let result = withUnsafePointer(to: &address) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.connect(descriptor, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard result == 0 else {
            let code = errno
            close(descriptor)
            throw SocketError.connectFailed(code)
        }
        return descriptor
    }

    private static func write(_ data: Data, to descriptor: Int32) throws {
        try data.withUnsafeBytes { buffer in
            guard let baseAddress = buffer.baseAddress else { return }
            var sent = 0
            while sent < buffer.count {
                let count = Darwin.write(descriptor, baseAddress.advanced(by: sent), buffer.count - sent)
                if count > 0 {
                    sent += count
                    continue
                }
                if count < 0, errno == EINTR {
                    continue
                }
                throw SocketError.writeFailed(errno)
            }
        }
    }

    private static func readLine(from descriptor: Int32) throws -> Data {
        var collected = Data()
        var buffer = [UInt8](repeating: 0, count: 512)
        let deadline = Date().addingTimeInterval(3)
        while Date() < deadline {
            var event = pollfd(fd: descriptor, events: Int16(POLLIN), revents: 0)
            let ready = poll(&event, 1, max(1, Int32(deadline.timeIntervalSinceNow * 1_000)))
            if ready == 0 { throw SocketError.timedOut }
            if ready < 0, errno == EINTR { continue }
            if ready < 0 { throw SocketError.readFailed(errno) }
            let count = Darwin.read(descriptor, &buffer, buffer.count)
            if count > 0 {
                collected.append(buffer, count: count)
                guard let newline = collected.firstIndex(of: UInt8(ascii: "\n")) else { continue }
                return collected.prefix(through: newline)
            }
            if count == 0 { throw SocketError.connectionClosed }
            if errno == EINTR { continue }
            throw SocketError.readFailed(errno)
        }
        throw SocketError.timedOut
    }

    private enum SocketError: Error {
        case createFailed(Int32)
        case pathTooLong
        case connectFailed(Int32)
        case writeFailed(Int32)
        case readFailed(Int32)
        case connectionClosed
        case timedOut
    }
}
