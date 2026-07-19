import Darwin
import Dispatch
import Foundation
import MuxyHookKit
import MuxyShared
import Testing

@Suite("Agent hook transport")
struct AgentHookTransportTests {
    @Test("ancestor traversal is nearest first and stops at init")
    func walksAncestors() {
        let parents: [Int32: Int32] = [99: 50, 50: 10, 10: 1]
        let result = AncestorProcessInspector.ancestorPIDs(startingAt: 99) { parents[$0] }

        #expect(result == [99, 50, 10, 1])
    }

    @Test("ancestor traversal stops cycles")
    func stopsAncestorCycles() {
        let result = AncestorProcessInspector.ancestorPIDs(startingAt: 9) { processID in
            processID == 9 ? 8 : 9
        }

        #expect(result == [9, 8])
    }

    @Test("socket client retries within a single shared budget")
    func retriesWithinSharedBudget() throws {
        let recorder = TransportRecorder(failuresBeforeSuccess: 2)
        let clock = ManualClock()
        let client = AgentHookSocketClient(
            maximumAttempts: 3,
            totalBudget: 0.4,
            retryDelay: 0.02,
            sendAttempt: recorder.send,
            sleep: { delay in
                recorder.recordSleep(delay)
                clock.advance(by: delay)
            },
            elapsed: clock.now
        )

        try client.send(Self.message, to: "/tmp/test.sock")

        #expect(recorder.attemptCount == 3)
        #expect(recorder.sleeps == [0.02, 0.02])
        #expect(try AgentHookWireCodec.decodeEventLine(#require(recorder.lastLine)) == Self.message)
    }

    @Test("each attempt receives only the remaining budget, never a fresh timeout")
    func attemptsShareRemainingBudget() throws {
        let recorder = TransportRecorder(failuresBeforeSuccess: .max)
        let clock = ManualClock()
        let client = AgentHookSocketClient(
            maximumAttempts: 3,
            totalBudget: 0.4,
            retryDelay: 0.02,
            sendAttempt: { path, line, budget in
                clock.advance(by: budget)
                try recorder.send(socketPath: path, line: line, timeout: budget)
            },
            sleep: { clock.advance(by: $0) },
            elapsed: clock.now
        )

        #expect(throws: (any Error).self) {
            try client.send(Self.message, to: "/tmp/test.sock")
        }

        #expect(recorder.attemptCount == 1)
        #expect(clock.now() <= 0.4)
    }

    @Test("an unresponsive server never exceeds the total budget across all retries")
    func totalBudgetBoundsUnresponsiveServer() throws {
        let recorder = TransportRecorder(failuresBeforeSuccess: .max)
        let clock = ManualClock()
        let stallPerAttempt = 0.25
        let client = AgentHookSocketClient(
            maximumAttempts: 3,
            totalBudget: 0.4,
            retryDelay: 0.02,
            sendAttempt: { path, line, budget in
                clock.advance(by: min(stallPerAttempt, budget))
                try recorder.send(socketPath: path, line: line, timeout: budget)
            },
            sleep: { clock.advance(by: $0) },
            elapsed: clock.now
        )

        #expect(throws: (any Error).self) {
            try client.send(Self.message, to: "/tmp/test.sock")
        }

        #expect(clock.now() <= 0.4)
        #expect(recorder.attemptCount == 2)
    }

    @Test("socket client accepts a v3 acknowledgement from a Unix socket")
    func acceptsAcknowledgement() throws {
        let socketPath = Self.temporarySocketPath()
        let listener = try Self.bindListener(at: socketPath)
        defer {
            close(listener)
            unlink(socketPath)
        }

        let acknowledgementTimeout: TimeInterval = 10
        let serverStarted = DispatchSemaphore(value: 0)
        let serverFinished = DispatchSemaphore(value: 0)
        let serverQueue = DispatchQueue(label: "com.muxy.tests.agent-hook-acknowledgement", qos: .userInitiated)
        serverQueue.async {
            serverStarted.signal()
            defer { serverFinished.signal() }
            let connection = accept(listener, nil, nil)
            guard connection >= 0 else { return }
            defer { close(connection) }
            _ = try? Self.readLine(from: connection)
            guard let acknowledgement = try? AgentHookWireCodec.encodeAcknowledgementLine(
                AgentHookAcknowledgement(ok: true)
            ) else { return }
            Self.write(acknowledgement, to: connection)
        }

        try #require(serverStarted.wait(timeout: .now() + acknowledgementTimeout) == .success)
        try AgentHookSocketClient.sendOnce(
            socketPath: socketPath,
            line: AgentHookWireCodec.encodeEventLine(Self.message),
            remainingBudget: acknowledgementTimeout
        )
        #expect(serverFinished.wait(timeout: .now() + acknowledgementTimeout) == .success)
    }

    @Test("socket client bounds writes when the connected peer stops reading")
    func boundsStalledWrites() throws {
        var descriptors = [Int32](repeating: -1, count: 2)
        try #require(socketpair(AF_UNIX, SOCK_STREAM, 0, &descriptors) == 0)
        defer {
            close(descriptors[0])
            close(descriptors[1])
        }
        var sendBufferSize: Int32 = 1_024
        try #require(setsockopt(
            descriptors[0],
            SOL_SOCKET,
            SO_SNDBUF,
            &sendBufferSize,
            socklen_t(MemoryLayout<Int32>.size)
        ) == 0)
        let deliveryTimeout: TimeInterval = 0.1
        let maximumElapsed = deliveryTimeout * 20
        let started = DispatchTime.now().uptimeNanoseconds

        #expect(throws: AgentHookSocketError.deliveryTimedOut) {
            try AgentHookSocketClient.sendConnected(
                descriptor: descriptors[0],
                line: Data(repeating: 1, count: 4 * 1_024 * 1_024),
                remainingBudget: deliveryTimeout
            )
        }

        let elapsed = Double(DispatchTime.now().uptimeNanoseconds - started) / 1_000_000_000
        #expect(elapsed < maximumElapsed)
    }

    @Test("runtime includes ancestors only when pane identity is unavailable")
    func runtimeFallsBackToAncestors() throws {
        let recorder = TransportRecorder()
        let client = AgentHookSocketClient(maximumAttempts: 1, sendAttempt: recorder.send)
        let runtime = AgentHookRuntime(
            environment: ["MUXY_SOCKET_PATH": "/tmp/test.sock"],
            socketClient: client,
            failureLogger: AgentHookFailureLogger(logFileURL: nil),
            ancestorPIDs: { [777, 42, 1] },
            timestamp: { 123 }
        )

        runtime.run(
            command: AgentHookCommand(provider: "opencode", providerTitle: "OpenCode", event: "session-end"),
            input: Data("{}".utf8)
        )

        let message = try AgentHookWireCodec.decodeEventLine(#require(recorder.lastLine))
        #expect(message.paneID == nil)
        #expect(message.pids == [777, 42, 1])
        #expect(message.ts == 123)
    }

    @Test("every retry re-sends the identical line so the event id is reused")
    func reusesEventIDAcrossRetries() throws {
        let recorder = TransportRecorder(failuresBeforeSuccess: 2)
        let client = AgentHookSocketClient(
            maximumAttempts: 3,
            retryDelay: 0,
            sendAttempt: recorder.send,
            sleep: { _ in }
        )
        let runtime = AgentHookRuntime(
            environment: [
                "MUXY_SOCKET_PATH": "/tmp/test.sock",
                "MUXY_PANE_ID": UUID().uuidString,
            ],
            socketClient: client,
            failureLogger: AgentHookFailureLogger(logFileURL: nil),
            ancestorPIDs: { [] },
            timestamp: { 123 }
        )

        runtime.run(
            command: AgentHookCommand(provider: "codex_hook", providerTitle: "Codex", event: "stop"),
            input: Data("{}".utf8)
        )

        let lines = recorder.lines
        #expect(lines.count == 3)
        #expect(Set(lines).count == 1)
        let firstLine = try #require(lines.first)
        let identifier = try #require(AgentHookWireCodec.decodeEventLine(firstLine).id)
        #expect(!identifier.isEmpty)
    }

    @Test("each logical event receives a distinct id")
    func generatesDistinctEventIDs() throws {
        let paneID = UUID().uuidString
        let identifiers = try (0 ..< 2).map { _ -> String in
            let recorder = TransportRecorder()
            let runtime = AgentHookRuntime(
                environment: ["MUXY_SOCKET_PATH": "/tmp/test.sock", "MUXY_PANE_ID": paneID],
                socketClient: AgentHookSocketClient(maximumAttempts: 1, sendAttempt: recorder.send),
                failureLogger: AgentHookFailureLogger(logFileURL: nil),
                ancestorPIDs: { [] },
                timestamp: { 123 }
            )
            runtime.run(
                command: AgentHookCommand(provider: "codex_hook", providerTitle: "Codex", event: "stop"),
                input: Data("{}".utf8)
            )
            let line = try #require(recorder.lastLine)
            return try #require(AgentHookWireCodec.decodeEventLine(line).id)
        }

        #expect(identifiers[0] != identifiers[1])
    }

    @Test("runtime logs the final delivery failure and returns normally")
    func logsFinalFailure() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-hook-log-" + UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let logURL = directory.appendingPathComponent("hooks.log")
        let recorder = TransportRecorder(failuresBeforeSuccess: .max)
        let client = AgentHookSocketClient(
            maximumAttempts: 3,
            retryDelay: 0,
            sendAttempt: recorder.send,
            sleep: { _ in }
        )
        let paneID = UUID().uuidString
        let runtime = AgentHookRuntime(
            environment: [
                "MUXY_SOCKET_PATH": "/tmp/missing.sock",
                "MUXY_PANE_ID": paneID,
            ],
            socketClient: client,
            failureLogger: AgentHookFailureLogger(logFileURL: logURL),
            ancestorPIDs: { [999] },
            timestamp: { 456 }
        )

        runtime.run(
            command: AgentHookCommand(provider: "codex_hook", providerTitle: "Codex", event: "stop"),
            input: Data("{}".utf8)
        )

        #expect(recorder.attemptCount == 3)
        let lines = try String(contentsOf: logURL, encoding: .utf8).split(separator: "\n")
        #expect(lines.count == 1)
        let record = try #require(JSONSerialization.jsonObject(with: Data(lines[0].utf8)) as? [String: Any])
        #expect(record["provider"] as? String == "codex_hook")
        #expect(record["event"] as? String == "stop")
        #expect(record["ts"] as? Int == 456)
        #expect(record["error"] as? String == "forcedFailure")

        let attributes = try FileManager.default.attributesOfItem(atPath: logURL.path)
        #expect((attributes[.posixPermissions] as? NSNumber)?.intValue == 0o600)
        let sent = try AgentHookWireCodec.decodeEventLine(#require(recorder.lastLine))
        #expect(sent.paneID == paneID)
        #expect(sent.pids.isEmpty)
    }

    @Test("failure logger bounds repeated delivery failures")
    func boundsFailureLog() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-hook-log-" + UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let logURL = directory.appendingPathComponent("hooks.log")
        let logger = AgentHookFailureLogger(logFileURL: logURL, maximumLogSize: 512)

        for index in 0 ..< 20 {
            logger.append(
                provider: "codex_hook",
                event: "stop-\(index)",
                error: AgentHookSocketError.invalidAcknowledgement,
                timestamp: Int64(index)
            )
        }

        let data = try Data(contentsOf: logURL)
        #expect(data.count <= 512)
        #expect(String(decoding: data, as: UTF8.self).contains(#""event":"stop-19""#))
    }

    private static let message = AgentHookEventMessage(
        provider: "claude_hook",
        paneID: UUID().uuidString,
        phase: .working,
        title: "",
        body: "",
        pids: [],
        ts: 100
    )

    private static func temporarySocketPath() -> String {
        "/tmp/mh-" + UUID().uuidString + ".sock"
    }

    private static func bindListener(at path: String) throws -> Int32 {
        let descriptor = socket(AF_UNIX, SOCK_STREAM, 0)
        guard descriptor >= 0 else { throw POSIXError(.EMFILE) }
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        let capacity = MemoryLayout.size(ofValue: address.sun_path)
        withUnsafeMutablePointer(to: &address.sun_path) { pointer in
            let destination = pointer.withMemoryRebound(to: CChar.self, capacity: capacity) { $0 }
            _ = path.withCString { strncpy(destination, $0, capacity - 1) }
        }
        let result = withUnsafePointer(to: &address) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                bind(descriptor, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard result == 0, listen(descriptor, 1) == 0 else {
            let error = POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
            close(descriptor)
            throw error
        }
        return descriptor
    }

    private static func readLine(from descriptor: Int32) throws -> Data {
        var data = Data()
        var byte: UInt8 = 0
        while byte != UInt8(ascii: "\n") {
            let count = Darwin.read(descriptor, &byte, 1)
            guard count == 1 else { throw POSIXError(.EIO) }
            data.append(byte)
        }
        return data
    }

    private static func write(_ data: Data, to descriptor: Int32) {
        data.withUnsafeBytes { buffer in
            guard let baseAddress = buffer.baseAddress else { return }
            var offset = 0
            while offset < buffer.count {
                let written = Darwin.write(descriptor, baseAddress.advanced(by: offset), buffer.count - offset)
                guard written > 0 else { return }
                offset += written
            }
        }
    }
}

private enum TransportTestError: Error {
    case forcedFailure
}

private final class ManualClock: @unchecked Sendable {
    private let lock = NSLock()
    private var current: TimeInterval = 0

    func now() -> TimeInterval {
        lock.withLock { current }
    }

    func advance(by interval: TimeInterval) {
        lock.lock()
        current += interval
        lock.unlock()
    }
}

private final class TransportRecorder: @unchecked Sendable {
    private let lock = NSLock()
    private let failuresBeforeSuccess: Int
    private var attempts = 0
    private var delays: [TimeInterval] = []
    private var line: Data?
    private var sentLines: [Data] = []

    init(failuresBeforeSuccess: Int = 0) {
        self.failuresBeforeSuccess = failuresBeforeSuccess
    }

    func send(socketPath _: String, line: Data, timeout _: TimeInterval) throws {
        lock.lock()
        attempts += 1
        self.line = line
        sentLines.append(line)
        let shouldFail = attempts <= failuresBeforeSuccess
        lock.unlock()
        if shouldFail {
            throw TransportTestError.forcedFailure
        }
    }

    func recordSleep(_ delay: TimeInterval) {
        lock.lock()
        delays.append(delay)
        lock.unlock()
    }

    var attemptCount: Int {
        lock.withLock { attempts }
    }

    var sleeps: [TimeInterval] {
        lock.withLock { delays }
    }

    var lastLine: Data? {
        lock.withLock { line }
    }

    var lines: [Data] {
        lock.withLock { sentLines }
    }
}
