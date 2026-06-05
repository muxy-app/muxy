import Foundation
import Testing

@testable import Muxy

@Suite("NotificationSocketServer readiness gate")
struct NotificationSocketReadyTests {
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
