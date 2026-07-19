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
