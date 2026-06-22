import Darwin
import Foundation
import Testing

@testable import Muxy

@Suite("NotificationSocketServer readiness gate", .serialized)
struct NotificationSocketReadyTests {
    @Test("test socket path does not use app support")
    func testSocketPathDoesNotUseAppSupport() {
        #expect(NotificationSocketServer.socketPath.contains(FileManager.default.temporaryDirectory.path))
        #expect(!NotificationSocketServer.socketPath.contains("Library/Application Support/Muxy"))
    }

    @Test("awaitReady resolves once the server finishes listening")
    func resolvesAfterStart() async throws {
        let server = NotificationSocketServer.shared
        server.start()

        try await Self.withTimeout(seconds: 5) {
            await server.awaitReady()
        }
    }

    @Test("awaitReady resolves immediately when listening already finished")
    func resolvesImmediatelyWhenAlreadyReady() async throws {
        let server = NotificationSocketServer.shared
        server.start()
        await server.awaitReady()

        try await Self.withTimeout(seconds: 1) {
            await server.awaitReady()
        }
    }

    @Test("repeated start keeps socket connectable")
    func repeatedStartKeepsSocketConnectable() async throws {
        let server = NotificationSocketServer.shared
        server.start()
        await server.awaitReady()
        server.start()
        await server.awaitReady()

        try Self.connectToSocket()
    }

    private static func connectToSocket() throws {
        let descriptor = socket(AF_UNIX, SOCK_STREAM, 0)
        #expect(descriptor >= 0)
        defer { close(descriptor) }

        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        withUnsafeMutablePointer(to: &addr.sun_path) { ptr in
            let bound = ptr.withMemoryRebound(to: CChar.self, capacity: 104) { $0 }
            _ = NotificationSocketServer.socketPath.withCString { strncpy(bound, $0, 103) }
        }

        let result = withUnsafePointer(to: &addr) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                connect(descriptor, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        #expect(result == 0)
    }

    private static func withTimeout(
        seconds: TimeInterval,
        _ operation: @escaping @Sendable () async -> Void
    ) async throws {
        try await withThrowingTaskGroup(of: Void.self) { group in
            group.addTask { await operation() }
            group.addTask {
                try await Task.sleep(for: .seconds(seconds))
                throw CancellationError()
            }
            try await group.next()
            group.cancelAll()
        }
    }
}
