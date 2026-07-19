import Darwin
import Foundation
import MuxyShared
import Testing

@testable import Muxy

@Suite("Agent hook pipeline end to end", .serialized)
@MainActor
struct AgentHookPipelineEndToEndTests {
    @Test("working then finished produces exactly one completion")
    func workingThenFinishedCompletesOnce() async throws {
        try await SharedNotificationStateGate.run {
            let harness = try PipelineHarness()
            defer { harness.tearDown() }

            try await harness.deliver(harness.event(phase: .working))
            try await harness.deliver(harness.event(phase: .finished, body: "All done"))

            #expect(AgentStatusStore.shared.status(forPane: harness.paneID) == .idle)
            #expect(AgentStatusStore.shared.isCompletionPending(forPane: harness.paneID))

            let delivered = harness.notifications()
            #expect(delivered.count == 1)
            #expect(delivered.first?.source == .aiProvider("claude"))
            #expect(delivered.first?.body == "All done")
        }
    }

    @Test("a waiting event reaches the notification path with the provider source")
    func waitingEventReachesNotificationPath() async throws {
        try await SharedNotificationStateGate.run {
            let harness = try PipelineHarness()
            defer { harness.tearDown() }

            try await harness.deliver(harness.event(phase: .working))
            try await harness.deliver(harness.event(
                phase: .waiting,
                title: "Claude Code",
                body: "Permission needed"
            ))

            #expect(AgentStatusStore.shared.status(forPane: harness.paneID) == .waiting)
            let delivered = harness.notifications()
            #expect(delivered.count == 1)
            #expect(delivered.first?.source == .aiProvider("claude"))
            #expect(delivered.first?.title == "Claude Code")
            #expect(delivered.first?.body == "Permission needed")
        }
    }

    @Test("a test-flagged event bypasses status and still notifies")
    func testEventBypassesStatus() async throws {
        try await SharedNotificationStateGate.run {
            let harness = try PipelineHarness()
            defer { harness.tearDown() }

            try await harness.deliver(AgentHookEventMessage(
                provider: "claude_hook",
                paneID: harness.paneID.uuidString,
                phase: .finished,
                title: "Notifications",
                body: "Hook pipeline is working",
                pids: [],
                ts: 1,
                test: true
            ))

            #expect(AgentStatusStore.shared.status(forPane: harness.paneID) == nil)
            let delivered = harness.notifications()
            #expect(delivered.count == 1)
            #expect(delivered.first?.title == "Notifications")
            #expect(delivered.first?.source == .aiProvider("claude"))
        }
    }

    @Test("pane identity resolves through the ancestor process chain")
    func pidFallbackResolvesPane() async throws {
        try await SharedNotificationStateGate.run {
            let harness = try PipelineHarness()
            defer { harness.tearDown() }

            TerminalViewRegistry.shared.overrideProcessIdentities([
                TerminalViewRegistry.PaneProcessIdentity(paneID: harness.paneID, processID: 4242),
            ])

            try await harness.deliver(AgentHookEventMessage(
                provider: "claude_hook",
                paneID: nil,
                phase: .working,
                title: "",
                body: "",
                pids: [9999, 4242, 1],
                ts: 1
            ))

            #expect(AgentStatusStore.shared.status(forPane: harness.paneID) == .working)
        }
    }

    @Test("malformed and unknown-version lines are rejected without an ack")
    func malformedLinesAreRejected() async throws {
        try await SharedNotificationStateGate.run {
            let harness = try PipelineHarness()
            defer { harness.tearDown() }

            try await harness.writeRaw(Data("{\"kind\":\"agent_event\"".utf8) + Data([10]))
            try await harness.writeRaw(try encoded(harness.event(phase: .working, version: 2)))

            #expect(!harness.acknowledged())
            #expect(AgentStatusStore.shared.status(forPane: harness.paneID) == nil)
        }
    }

    @Test("staged bridge binary drives the pipeline over a bound socket")
    func stagedBinaryDrivesPipeline() async throws {
        try await SharedNotificationStateGate.run {
            let harness = try PipelineHarness()
            defer { harness.tearDown() }

            let binaryURL = RepositoryRoot.find().appendingPathComponent(".build/debug/muxy-hook")
            try #require(FileManager.default.isExecutableFile(atPath: binaryURL.path))

            try await harness.runHookBinary(
                binaryURL: binaryURL,
                arguments: [
                    "agent-event",
                    "--provider", "claude_hook",
                    "--provider-title", "Claude Code",
                    "--event", "stop",
                ],
                input: "{\"last_assistant_message\":\"Bridge complete\"}"
            )

            try await harness.waitForNotification()
            let delivered = harness.notifications()
            #expect(delivered.count == 1)
            #expect(delivered.first?.source == .aiProvider("claude"))
            #expect(AgentStatusStore.shared.status(forPane: harness.paneID) == .idle)
        }
    }

    private func encoded(_ message: AgentHookEventMessage) throws -> Data {
        try AgentHookWireCodec.encodeEventLine(message)
    }
}

@MainActor
private final class PipelineHarness {
    let paneID: UUID
    let projectID = UUID()
    let worktreeID = UUID()
    private let socketPath: String
    private let server: NotificationSocketServer

    init() throws {
        socketPath = FileManager.default.temporaryDirectory
            .appendingPathComponent("ahp-\(UUID().uuidString.prefix(8)).sock")
            .path
        server = NotificationSocketServer(socketPath: socketPath)

        let appState = AppState(
            selectionStore: SelectionStub(),
            terminalViews: TerminalViewStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = TabArea(projectPath: "/tmp/pipeline")
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = .tabArea(area)
        appState.focusedAreaID[key] = area.id
        paneID = area.tabs.last!.content.pane!.id

        AgentStatusStore.shared.removePane(paneID)
        NotificationStore.shared.clear()
        NotificationStore.shared.appState = appState
        NotificationStore.shared.worktreeStore = WorktreeStore(
            persistence: WorktreePersistenceStub(),
            projects: []
        )
    }

    func tearDown() {
        if rawDescriptor >= 0 { close(rawDescriptor) }
        server.stop()
        AgentStatusStore.shared.removePane(paneID)
        NotificationStore.shared.clear()
        TerminalViewRegistry.shared.overrideProcessIdentities(nil)
        unlink(socketPath)
    }

    func event(
        phase: AgentHookPhase,
        title: String = "",
        body: String = "",
        version: Int = AgentHookProtocol.version
    ) -> AgentHookEventMessage {
        AgentHookEventMessage(
            v: version,
            provider: "claude_hook",
            paneID: paneID.uuidString,
            phase: phase,
            title: title,
            body: body,
            pids: [],
            ts: 1
        )
    }

    func notifications() -> [MuxyNotification] {
        NotificationStore.shared.notifications.filter { $0.projectID == projectID }
    }

    func deliver(_ message: AgentHookEventMessage) async throws {
        try await ensureListening()
        let descriptor = try SocketProbe.connect(to: socketPath)
        defer { close(descriptor) }
        try SocketProbe.write(AgentHookWireCodec.encodeEventLine(message), to: descriptor)
        _ = try SocketProbe.readLine(from: descriptor)
        try await drainMainQueue()
    }

    func writeRaw(_ data: Data) async throws {
        try await ensureListening()
        if rawDescriptor >= 0 { close(rawDescriptor) }
        rawDescriptor = try SocketProbe.connect(to: socketPath)
        try SocketProbe.write(data, to: rawDescriptor)
        try await drainMainQueue()
    }

    func acknowledged() -> Bool {
        guard rawDescriptor >= 0 else { return false }
        return (try? SocketProbe.readLine(from: rawDescriptor, timeout: 0.5)) != nil
    }

    func runHookBinary(binaryURL: URL, arguments: [String], input: String) async throws {
        try await ensureListening()
        let process = Process()
        process.executableURL = binaryURL
        process.arguments = arguments
        var environment = ProcessInfo.processInfo.environment
        environment["MUXY_SOCKET_PATH"] = socketPath
        environment["MUXY_PANE_ID"] = paneID.uuidString
        process.environment = environment
        let standardInput = Pipe()
        process.standardInput = standardInput
        process.standardOutput = Pipe()
        process.standardError = Pipe()

        try process.run()
        standardInput.fileHandleForWriting.write(Data(input.utf8))
        try? standardInput.fileHandleForWriting.close()
        let deadline = Date().addingTimeInterval(5)
        while process.isRunning, Date() < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        if process.isRunning {
            process.terminate()
        }
    }

    func waitForNotification() async throws {
        let deadline = Date().addingTimeInterval(5)
        while Date() < deadline {
            try await drainMainQueue()
            if !notifications().isEmpty { return }
            try await Task.sleep(for: .milliseconds(20))
        }
    }

    private func ensureListening() async throws {
        guard !didStart else { return }
        server.start()
        await server.awaitReady()
        didStart = true
    }

    private func drainMainQueue() async throws {
        for _ in 0 ..< 4 {
            await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
                DispatchQueue.main.async { continuation.resume() }
            }
        }
    }

    private var didStart = false
    private var rawDescriptor: Int32 = -1
}

private enum SocketProbe {
    static func connect(to path: String) throws -> Int32 {
        let descriptor = socket(AF_UNIX, SOCK_STREAM, 0)
        guard descriptor >= 0 else { throw Failure.createFailed }
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        let capacity = MemoryLayout.size(ofValue: address.sun_path)
        guard path.utf8.count < capacity else {
            close(descriptor)
            throw Failure.pathTooLong
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
            close(descriptor)
            throw Failure.connectFailed
        }
        return descriptor
    }

    static func write(_ data: Data, to descriptor: Int32) throws {
        try data.withUnsafeBytes { buffer in
            guard let baseAddress = buffer.baseAddress else { return }
            var sent = 0
            while sent < buffer.count {
                let count = Darwin.write(descriptor, baseAddress.advanced(by: sent), buffer.count - sent)
                if count > 0 {
                    sent += count
                    continue
                }
                if count < 0, errno == EINTR { continue }
                throw Failure.writeFailed
            }
        }
    }

    static func readLine(from descriptor: Int32, timeout: TimeInterval = 3) throws -> Data {
        var collected = Data()
        var buffer = [UInt8](repeating: 0, count: 512)
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            var event = pollfd(fd: descriptor, events: Int16(POLLIN), revents: 0)
            let ready = poll(&event, 1, max(1, Int32(deadline.timeIntervalSinceNow * 1_000)))
            if ready == 0 { throw Failure.timedOut }
            if ready < 0, errno == EINTR { continue }
            if ready < 0 { throw Failure.readFailed }
            let count = Darwin.read(descriptor, &buffer, buffer.count)
            if count > 0 {
                collected.append(buffer, count: count)
                guard let newline = collected.firstIndex(of: UInt8(ascii: "\n")) else { continue }
                return collected.prefix(through: newline)
            }
            if count == 0 { throw Failure.closed }
            if errno == EINTR { continue }
            throw Failure.readFailed
        }
        throw Failure.timedOut
    }

    enum Failure: Error {
        case createFailed
        case pathTooLong
        case connectFailed
        case writeFailed
        case readFailed
        case closed
        case timedOut
    }
}

@MainActor
private final class SelectionStub: ActiveProjectSelectionStoring {
    private var activeProjectID: UUID?
    private var activeWorktreeIDs: [UUID: UUID] = [:]
    func loadActiveProjectID() -> UUID? { activeProjectID }
    func saveActiveProjectID(_ id: UUID?) { activeProjectID = id }
    func loadActiveWorktreeIDs() -> [UUID: UUID] { activeWorktreeIDs }
    func saveActiveWorktreeIDs(_ ids: [UUID: UUID]) { activeWorktreeIDs = ids }
}

@MainActor
private final class TerminalViewStub: TerminalViewRemoving {
    func removeView(for _: UUID) {}
    func needsConfirmQuit(for _: UUID) -> Bool { false }
}

private final class WorkspacePersistenceStub: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
}

private final class WorktreePersistenceStub: WorktreePersisting {
    func loadWorktrees(projectID _: UUID) throws -> [Worktree] { [] }
    func saveWorktrees(_: [Worktree], projectID _: UUID) throws {}
    func removeWorktrees(projectID _: UUID) throws {}
}
