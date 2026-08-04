import Darwin
import Foundation
import MuxySessionProtocol
import Testing

@testable import Muxy

@Suite("SessionDaemon lifecycle", .serialized, .enabled(if: SessionDaemonHarness.binaryURL != nil))
struct SessionDaemonLifecycleTests {
    private static let idleTimeoutMilliseconds = 100

    private func makeIdentifier() throws -> SessionIdentifier {
        try #require(SessionIdentifier(uuidString: UUID().uuidString))
    }

    @Test("starts a fresh daemon after an idle one has exited")
    func attachClientStartsDaemonAfterIdleExit() throws {
        let harness = try SessionDaemonHarness()
        defer { harness.stop() }
        try harness.start(idleTimeoutMilliseconds: Self.idleTimeoutMilliseconds)

        let deadline = Date().addingTimeInterval(5)
        while FileManager.default.fileExists(atPath: harness.socketPath), Date() < deadline {
            usleep(20_000)
        }
        #expect(!FileManager.default.fileExists(atPath: harness.socketPath))

        let result = try harness.runAttachClient(
            identifier: makeIdentifier(),
            command: "echo RESTARTED",
            timeout: 15
        )
        #expect(result.output.contains("RESTARTED"))
        #expect(result.status == 0)
    }

    @Test("starts a daemon on demand and runs the session command")
    func attachClientStartsDaemonOnDemand() throws {
        let harness = try SessionDaemonHarness()
        defer { harness.stop() }

        let result = try harness.runAttachClient(
            identifier: makeIdentifier(),
            command: "echo CLIENT_READY",
            timeout: 15
        )
        #expect(result.output.contains("CLIENT_READY"))
        #expect(result.status == 0)
    }

    @Test("fails immediately when the daemon binary cannot be launched")
    func attachClientFailsFastForMissingDaemonBinary() throws {
        let harness = try SessionDaemonHarness()
        defer { harness.stop() }

        let result = try harness.runAttachClient(
            identifier: makeIdentifier(),
            command: "",
            timeout: 5,
            daemonBinaryPath: harness.directory.appendingPathComponent("missing-muxy-session").path
        )
        let launchFailureCount = result.output.components(separatedBy: "could not start the session daemon").count - 1
        #expect(result.status == 93)
        #expect(launchFailureCount == 1)
    }

    @Test("keeps trying after the first daemon loses the startup race")
    func attachClientRetriesAfterLostStartupRace() throws {
        let harness = try SessionDaemonHarness()
        defer { harness.stop() }

        let lock = open(harness.socketPath + ".lock", O_CREAT | O_RDWR, 0o600)
        try #require(lock >= 0)
        try #require(flock(lock, LOCK_EX | LOCK_NB) == 0)
        DispatchQueue.global().asyncAfter(deadline: .now() + 1.2) {
            flock(lock, LOCK_UN)
            close(lock)
        }

        let result = try harness.runAttachClient(
            identifier: makeIdentifier(),
            command: "echo RACED",
            timeout: 15
        )
        #expect(result.output.contains("RACED"))
        #expect(result.status == 0)
    }

    @Test("keeps daemons on separate sockets fully isolated")
    func isolatesDaemonsOnSeparateSockets() throws {
        let first = try SessionDaemonHarness()
        defer { first.stop() }
        try first.start()
        let second = try SessionDaemonHarness()
        defer { second.stop() }
        try second.start()

        let firstAttached = try #require(SessionTestConnection(socketPath: first.socketPath))
        defer { firstAttached.close() }
        firstAttached.send(first.attachRequest(identifier: try makeIdentifier(), command: "sh -c 'sleep 30'"))
        _ = try #require(firstAttached.waitForFrame(timeout: 5) { $0.kind == .attached })

        let secondAttached = try #require(SessionTestConnection(socketPath: second.socketPath))
        defer { secondAttached.close() }
        secondAttached.send(second.attachRequest(identifier: try makeIdentifier(), command: "sh -c 'sleep 30'"))
        _ = try #require(secondAttached.waitForFrame(timeout: 5) { $0.kind == .attached })

        let firstControl = try #require(SessionTestConnection(socketPath: first.socketPath))
        defer { firstControl.close() }
        firstControl.send(SessionFrame(kind: .killAll))
        _ = try #require(firstControl.waitForFrame(timeout: 5) { $0.kind == .acknowledged })

        let secondControl = try #require(SessionTestConnection(socketPath: second.socketPath))
        defer { secondControl.close() }
        secondControl.send(SessionFrame(kind: .list))
        let listed = try #require(secondControl.waitForFrame(timeout: 5) { $0.kind == .sessions })
        #expect(try SessionDescriptor.decodeList(listed.payload).count == 1)
    }

    @Test("recovers when the socket path holds a stale file")
    func attachClientRecoversFromStaleSocketPath() throws {
        let harness = try SessionDaemonHarness()
        defer { harness.stop() }
        try Data().write(to: URL(fileURLWithPath: harness.socketPath))

        let result = try harness.runAttachClient(
            identifier: makeIdentifier(),
            command: "echo RECOVERED",
            timeout: 15
        )
        #expect(result.output.contains("RECOVERED"))
        #expect(result.status == 0)
    }
}
