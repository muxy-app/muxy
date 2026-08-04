import Darwin
import Foundation
import MuxySessionProtocol
import os

private let logger = Logger(subsystem: "app.muxy", category: "PersistentSession")

enum PersistentSessionLookup: Equatable, Sendable {
    case found(SessionDescriptor)
    case absent
    case unreachable
}

struct PersistentSessionControlClient: Sendable {
    static let defaultTimeout: TimeInterval = 2

    let socketPath: String

    func list(timeout: TimeInterval = defaultTimeout) -> [SessionDescriptor] {
        descriptors(for: SessionFrame(kind: .list), timeout: timeout)
    }

    func info(identifier: SessionIdentifier, timeout: TimeInterval = defaultTimeout) -> SessionDescriptor? {
        guard case let .found(descriptor) = lookup(identifier: identifier, timeout: timeout) else { return nil }
        return descriptor
    }

    func lookup(identifier: SessionIdentifier, timeout: TimeInterval = defaultTimeout) -> PersistentSessionLookup {
        let response = exchange(
            SessionFrame(kind: .info, payload: SessionIdentifierPayload.encode(identifier)),
            expecting: .sessions,
            timeout: timeout
        )
        guard case let .frame(frame) = response else { return .unreachable }
        do {
            guard let descriptor = try SessionDescriptor.decodeList(frame.payload).first else { return .absent }
            return .found(descriptor)
        } catch {
            logger.error("failed to decode the session lookup: \(String(describing: error))")
            return .unreachable
        }
    }

    @discardableResult
    func kill(identifier: SessionIdentifier, timeout: TimeInterval = defaultTimeout) -> Bool {
        acknowledged(
            SessionFrame(kind: .kill, payload: SessionIdentifierPayload.encode(identifier)),
            timeout: timeout
        )
    }

    @discardableResult
    func killAll(timeout: TimeInterval = defaultTimeout) -> Bool {
        acknowledged(SessionFrame(kind: .killAll), timeout: timeout)
    }

    private enum ExchangeResult {
        case frame(SessionFrame)
        case unreachable
    }

    private func descriptors(for frame: SessionFrame, timeout: TimeInterval) -> [SessionDescriptor] {
        guard case let .frame(response) = exchange(frame, expecting: .sessions, timeout: timeout) else { return [] }
        do {
            return try SessionDescriptor.decodeList(response.payload)
        } catch {
            logger.error("failed to decode the session list: \(String(describing: error))")
            return []
        }
    }

    private func acknowledged(_ frame: SessionFrame, timeout: TimeInterval) -> Bool {
        guard case .frame = exchange(frame, expecting: .acknowledged, timeout: timeout) else { return false }
        return true
    }

    private func exchange(
        _ frame: SessionFrame,
        expecting kind: SessionFrameKind,
        timeout: TimeInterval
    ) -> ExchangeResult {
        guard let descriptor = PersistentSessionSocket.connect(path: socketPath) else { return .unreachable }
        defer { close(descriptor) }

        let deadline = Date().addingTimeInterval(timeout)
        guard PersistentSessionSocket.write(descriptor, bytes: frame.encoded(), deadline: deadline) else {
            return .unreachable
        }

        var decoder = SessionFrameDecoder()
        while Date() < deadline {
            guard let bytes = PersistentSessionSocket.read(descriptor, deadline: deadline) else { return .unreachable }
            decoder.push(bytes)
            while true {
                let next: SessionFrame?
                do {
                    next = try decoder.next()
                } catch {
                    logger.error("session daemon sent a malformed frame")
                    return .unreachable
                }
                guard let next else { break }
                if next.kind == kind {
                    return .frame(next)
                }
            }
        }
        return .unreachable
    }
}

enum PersistentSessionSocket {
    static func connect(path: String) -> Int32? {
        guard PersistentSessionPaths.fits(path) else { return nil }
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        let capacity = MemoryLayout.size(ofValue: address.sun_path)
        let bytes = Array(path.utf8)
        guard bytes.count < capacity else { return nil }
        withUnsafeMutablePointer(to: &address.sun_path) { pointer in
            pointer.withMemoryRebound(to: CChar.self, capacity: capacity) { destination in
                for (index, byte) in bytes.enumerated() {
                    destination[index] = CChar(bitPattern: byte)
                }
                destination[bytes.count] = 0
            }
        }
        address.sun_len = UInt8(MemoryLayout<sockaddr_un>.size)

        let descriptor = socket(AF_UNIX, SOCK_STREAM, 0)
        guard descriptor >= 0 else { return nil }
        let connected = withUnsafePointer(to: &address) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) { casted in
                Darwin.connect(descriptor, casted, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard connected == 0 else {
            close(descriptor)
            return nil
        }
        return descriptor
    }

    static func write(_ descriptor: Int32, bytes: [UInt8], deadline: Date) -> Bool {
        var offset = 0
        while offset < bytes.count {
            guard wait(descriptor, for: Int16(POLLOUT), deadline: deadline) else { return false }
            let written = bytes.withUnsafeBytes { pointer -> Int in
                guard let base = pointer.baseAddress else { return -1 }
                return Darwin.write(descriptor, base.advanced(by: offset), bytes.count - offset)
            }
            if written > 0 {
                offset += written
                continue
            }
            guard written < 0, errno == EINTR || errno == EAGAIN else { return false }
        }
        return true
    }

    static func read(_ descriptor: Int32, deadline: Date) -> [UInt8]? {
        guard wait(descriptor, for: Int16(POLLIN), deadline: deadline) else { return nil }
        let capacity = 64 * 1024
        var buffer = [UInt8](repeating: 0, count: capacity)
        let received = buffer.withUnsafeMutableBytes { pointer in
            Darwin.read(descriptor, pointer.baseAddress, capacity)
        }
        guard received > 0 else { return nil }
        return Array(buffer[0 ..< received])
    }

    private static func wait(_ descriptor: Int32, for events: Int16, deadline: Date) -> Bool {
        let remaining = deadline.timeIntervalSinceNow
        guard remaining > 0 else { return false }
        var descriptors = [pollfd(fd: descriptor, events: events, revents: 0)]
        return poll(&descriptors, 1, Int32(remaining * 1000)) > 0
    }
}
