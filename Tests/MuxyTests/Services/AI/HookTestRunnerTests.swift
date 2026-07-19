import Darwin
import Foundation
import MuxyShared
import Testing

@testable import Muxy

@Suite("HookTestRunner")
struct HookTestRunnerTests {
    @Test("arguments include the test flag and provider tags")
    func argumentsIncludeTestFlag() {
        let arguments = HookTestRunner.arguments(providerSocketType: "claude_hook", providerTitle: "Claude Code")
        #expect(arguments == [
            "agent-event",
            "--provider", "claude_hook",
            "--provider-title", "Claude Code",
            "--event", "test",
            "--test",
        ])
    }

    @Test("interpret maps a clean exit to passed")
    func interpretMapsCleanExit() {
        let outcome = HookTestRunner.ProcessOutcome(terminationStatus: 0, standardError: "")
        #expect(HookTestRunner.interpret(outcome) == .passed)
    }

    @Test("interpret surfaces stderr on failure")
    func interpretSurfacesStderr() {
        let outcome = HookTestRunner.ProcessOutcome(terminationStatus: 1, standardError: "deliveryTimedOut\n")
        #expect(HookTestRunner.interpret(outcome) == .failed("deliveryTimedOut"))
    }

    @Test("interpret falls back to status when stderr is empty")
    func interpretFallsBackToStatus() {
        let outcome = HookTestRunner.ProcessOutcome(terminationStatus: 2, standardError: "")
        #expect(HookTestRunner.interpret(outcome) == .failed("Hook exited with status 2"))
    }

    @Test("run reports failure when the binary is not staged")
    func runReportsMissingBinary() {
        let runner = HookTestRunner(
            binaryPath: "/does/not/exist",
            socketPath: "/tmp/whatever.sock",
            fileExists: { _ in false },
            runner: { _, _, _, _ in HookTestRunner.ProcessOutcome(terminationStatus: 0, standardError: "") }
        )
        #expect(runner.run(providerSocketType: "claude_hook", providerTitle: "Claude") == .failed("Hook binary is not staged"))
    }

    @Test("run passes socket path in the environment and interprets the outcome")
    func runPassesSocketPathAndInterprets() {
        let capturedEnvironment = EnvironmentCapture()
        let runner = HookTestRunner(
            binaryPath: "/staged/muxy-hook",
            socketPath: "/tmp/live.sock",
            fileExists: { _ in true },
            runner: { _, arguments, environment, _ in
                capturedEnvironment.store(environment: environment, arguments: arguments)
                return HookTestRunner.ProcessOutcome(terminationStatus: 0, standardError: "")
            }
        )

        #expect(runner.run(providerSocketType: "codex_hook", providerTitle: "Codex") == .passed)
        #expect(capturedEnvironment.environment["MUXY_SOCKET_PATH"] == "/tmp/live.sock")
        #expect(capturedEnvironment.arguments.contains("--test"))
    }

    @Test("real muxy-hook binary traverses the socket and receives an ack")
    func realBinaryReceivesAck() async throws {
        let binaryPath = try Self.hookBinaryPath()
        let socketPath = Self.temporarySocketPath()
        let server = NotificationSocketServer(socketPath: socketPath)
        server.start()
        await server.awaitReady()
        defer {
            server.stop()
            unlink(socketPath)
        }

        let runner = HookTestRunner(
            binaryPath: binaryPath,
            socketPath: socketPath,
            fileExists: { _ in true },
            timeout: 5
        )

        let result = runner.run(providerSocketType: "claude_hook", providerTitle: "Claude Code")
        #expect(result == .passed)
    }

    @Test("real muxy-hook binary reports failure when nothing is listening")
    func realBinaryReportsFailureWithoutServer() throws {
        let binaryPath = try Self.hookBinaryPath()
        let socketPath = Self.temporarySocketPath()

        let runner = HookTestRunner(
            binaryPath: binaryPath,
            socketPath: socketPath,
            fileExists: { _ in true },
            timeout: 5
        )

        let result = runner.run(providerSocketType: "claude_hook", providerTitle: "Claude Code")
        guard case .failed = result else {
            Issue.record("expected failure when no server is listening, got \(result)")
            return
        }
    }

    @Test("large stderr does not stall the wait loop or misreport a clean exit")
    func largeStderrDoesNotDeadlock() throws {
        let script = try Self.makeScript(
            body: """
            head -c 400000 /dev/zero | tr '\\0' 'x' >&2
            exit 0
            """
        )
        defer { try? FileManager.default.removeItem(atPath: script) }

        let outcome = try HookTestRunner.runProcess(
            binaryPath: script,
            arguments: [],
            environment: [:],
            timeout: 20
        )

        #expect(outcome.terminationStatus == 0)
        #expect(outcome.standardError.count == 400_000)
    }

    @Test("a hung child is reaped and reported as a timeout")
    func hungChildIsReaped() throws {
        let script = try Self.makeScript(body: "sleep 120")
        defer { try? FileManager.default.removeItem(atPath: script) }

        let outcome = try HookTestRunner.runProcess(
            binaryPath: script,
            arguments: [],
            environment: [:],
            timeout: 0.2
        )

        #expect(outcome.terminationStatus == -1)
        #expect(outcome.standardError == "Hook timed out")
        #expect(Self.lingeringSleepCount(matching: script) == 0)
    }

    private static func lingeringSleepCount(matching script: String) -> Int {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/ps")
        process.arguments = ["-A", "-o", "command"]
        let pipe = Pipe()
        process.standardOutput = pipe
        guard (try? process.run()) != nil else { return 0 }
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        let output = String(data: data, encoding: .utf8) ?? ""
        return output.split(separator: "\n").filter { $0.contains(script) }.count
    }

    private static func makeScript(body: String) throws -> String {
        let path = FileManager.default.temporaryDirectory
            .appendingPathComponent("htr-script-\(UUID().uuidString.prefix(8)).sh")
        try "#!/bin/bash\n\(body)\n".write(to: path, atomically: true, encoding: .utf8)
        try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: path.path)
        return path.path
    }

    private static func hookBinaryPath() throws -> String {
        let candidate = RepositoryRoot.find().appendingPathComponent(".build/debug/muxy-hook")
        guard FileManager.default.isExecutableFile(atPath: candidate.path) else {
            throw HookBinaryError.notBuilt(candidate.path)
        }
        return candidate.path
    }

    private static func temporarySocketPath() -> String {
        FileManager.default.temporaryDirectory
            .appendingPathComponent("htr-\(UUID().uuidString.prefix(8)).sock")
            .path
    }

    private enum HookBinaryError: Error {
        case notBuilt(String)
    }

    private final class EnvironmentCapture: @unchecked Sendable {
        private let lock = NSLock()
        private var storedEnvironment: [String: String] = [:]
        private var storedArguments: [String] = []

        var environment: [String: String] { lock.withLock { storedEnvironment } }
        var arguments: [String] { lock.withLock { storedArguments } }

        func store(environment: [String: String], arguments: [String]) {
            lock.withLock {
                storedEnvironment = environment
                storedArguments = arguments
            }
        }
    }
}
