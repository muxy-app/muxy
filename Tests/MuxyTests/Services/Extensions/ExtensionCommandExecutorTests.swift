import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionCommandExecutor")
struct ExtensionCommandExecutorTests {
    @Test("argv form captures stdout")
    func argvCapturesStdout() async throws {
        let request = ExecRequest(
            argv: ["/bin/echo", "hello world"],
            shell: nil,
            cwd: nil,
            env: nil,
            stdin: nil,
            timeoutMs: nil
        )
        let result = try await ExtensionCommandExecutor.runUnchecked(
            request: request,
            extensionID: "test",
            defaultCwd: nil
        )
        #expect(result.exitCode == 0)
        #expect(result.stdout.contains("hello world"))
        #expect(result.timedOut == false)
    }

    @Test("shell form runs pipes")
    func shellRunsPipes() async throws {
        let request = ExecRequest(
            argv: nil,
            shell: "echo one two three | wc -w",
            cwd: nil,
            env: nil,
            stdin: nil,
            timeoutMs: nil
        )
        let result = try await ExtensionCommandExecutor.runUnchecked(
            request: request,
            extensionID: "test",
            defaultCwd: nil
        )
        #expect(result.exitCode == 0)
        #expect(result.stdout.trimmingCharacters(in: .whitespacesAndNewlines) == "3")
    }

    @Test("nonzero exit code is reported")
    func nonzeroExit() async throws {
        let request = ExecRequest(
            argv: ["/usr/bin/false"],
            shell: nil,
            cwd: nil,
            env: nil,
            stdin: nil,
            timeoutMs: nil
        )
        let result = try await ExtensionCommandExecutor.runUnchecked(
            request: request,
            extensionID: "test",
            defaultCwd: nil
        )
        #expect(result.exitCode != 0)
    }

    @Test("timeout terminates a long-running command")
    func timeoutTerminates() async throws {
        let request = ExecRequest(
            argv: ["/bin/sleep", "10"],
            shell: nil,
            cwd: nil,
            env: nil,
            stdin: nil,
            timeoutMs: 200
        )
        let result = try await ExtensionCommandExecutor.runUnchecked(
            request: request,
            extensionID: "test",
            defaultCwd: nil
        )
        #expect(result.timedOut == true)
        #expect(result.exitCode != 0)
    }

    @Test("cancellable exec resolves normal command result")
    func cancellableExecResolves() async throws {
        let box = ExecCompletionBox()
        let request = ExecRequest(
            argv: ["/bin/echo", "async hello"],
            shell: nil,
            cwd: nil,
            env: nil,
            stdin: nil,
            timeoutMs: nil
        )
        let jobID = ExtensionCommandExecutor.startCancelableUnchecked(
            request: request,
            extensionID: "test",
            defaultCwd: nil
        ) { result in
            box.complete(result)
        }

        let result = try await box.wait().get()
        #expect(!jobID.isEmpty)
        #expect(result.exitCode == 0)
        #expect(result.stdout.contains("async hello"))
        #expect(result.timedOut == false)
    }

    @Test("cancelExec terminates long-running process")
    func cancelExecTerminatesLongRunningProcess() async throws {
        let box = ExecCompletionBox()
        let marker = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-exec-started-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: marker) }
        let request = ExecRequest(
            argv: nil,
            shell: "printf started > \(marker.path); while true; do sleep 1; done",
            cwd: nil,
            env: nil,
            stdin: nil,
            timeoutMs: 0
        )
        let jobID = ExtensionCommandExecutor.startCancelableUnchecked(
            request: request,
            extensionID: "test",
            defaultCwd: nil
        ) { result in
            box.complete(result)
        }

        try await waitForFile(at: marker)
        #expect(ExtensionCommandExecutor.cancelExec(jobID: jobID))

        do {
            _ = try await box.wait().get()
            Issue.record("expected cancellation")
        } catch ExecError.cancelled {
        } catch {
            Issue.record("expected ExecError.cancelled, got \(error)")
        }
    }

    @Test("cancel after completion is a no-op")
    func cancelAfterCompletionIsNoOp() async throws {
        let box = ExecCompletionBox()
        let request = ExecRequest(
            argv: ["/bin/echo", "done"],
            shell: nil,
            cwd: nil,
            env: nil,
            stdin: nil,
            timeoutMs: nil
        )
        let jobID = ExtensionCommandExecutor.startCancelableUnchecked(
            request: request,
            extensionID: "test",
            defaultCwd: nil
        ) { result in
            box.complete(result)
        }

        _ = try await box.wait().get()
        #expect(!ExtensionCommandExecutor.cancelExec(jobID: jobID))
    }

    @Test("timeout after cancel does not finish twice")
    func timeoutAfterCancelDoesNotFinishTwice() async throws {
        let box = ExecCompletionBox()
        let marker = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-exec-started-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: marker) }
        let request = ExecRequest(
            argv: nil,
            shell: "printf started > \(marker.path); while true; do sleep 1; done",
            cwd: nil,
            env: nil,
            stdin: nil,
            timeoutMs: 5000
        )
        let jobID = ExtensionCommandExecutor.startCancelableUnchecked(
            request: request,
            extensionID: "test",
            defaultCwd: nil
        ) { result in
            box.complete(result)
        }

        try await waitForFile(at: marker)
        #expect(ExtensionCommandExecutor.cancelExec(jobID: jobID))
        _ = await box.wait()
        try await Task.sleep(for: .milliseconds(5200))
        #expect(box.count == 1)
    }

    @Test("stdin is piped to the child")
    func stdinPiped() async throws {
        let request = ExecRequest(
            argv: ["/bin/cat"],
            shell: nil,
            cwd: nil,
            env: nil,
            stdin: "hello from stdin",
            timeoutMs: nil
        )
        let result = try await ExtensionCommandExecutor.runUnchecked(
            request: request,
            extensionID: "test",
            defaultCwd: nil
        )
        #expect(result.exitCode == 0)
        #expect(result.stdout == "hello from stdin")
    }

    @Test("defaultCwd is used when cwd is not provided")
    func defaultCwdUsed() async throws {
        let tempDir = FileManager.default.temporaryDirectory.path
        let request = ExecRequest(
            argv: ["/bin/pwd"],
            shell: nil,
            cwd: nil,
            env: nil,
            stdin: nil,
            timeoutMs: nil
        )
        let result = try await ExtensionCommandExecutor.runUnchecked(
            request: request,
            extensionID: "test",
            defaultCwd: tempDir
        )
        let pwd = result.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
        let normalized = URL(fileURLWithPath: pwd).resolvingSymlinksInPath().path
        let expected = URL(fileURLWithPath: tempDir).resolvingSymlinksInPath().path
        #expect(normalized == expected)
    }

    @Test("invalid request rejects with ExecError")
    func invalidRequest() async {
        let request = ExecRequest(
            argv: [],
            shell: nil,
            cwd: nil,
            env: nil,
            stdin: nil,
            timeoutMs: nil
        )
        do {
            _ = try await ExtensionCommandExecutor.runUnchecked(
                request: request,
                extensionID: "test",
                defaultCwd: nil
            )
            Issue.record("expected throw")
        } catch is ExecError {
        } catch {
            Issue.record("expected ExecError, got \(error)")
        }
    }
}

private func waitForFile(at url: URL) async throws {
    for _ in 0 ..< 100 {
        if FileManager.default.fileExists(atPath: url.path) {
            return
        }
        try await Task.sleep(for: .milliseconds(20))
    }
    Issue.record("expected marker file at \(url.path)")
}

private final class ExecCompletionBox: @unchecked Sendable {
    private let lock = NSLock()
    private var stored: Result<ExecResult, Error>?
    private var completions = 0

    var count: Int {
        lock.lock()
        defer { lock.unlock() }
        return completions
    }

    func complete(_ result: Result<ExecResult, Error>) {
        lock.lock()
        completions += 1
        if stored == nil {
            stored = result
        }
        lock.unlock()
    }

    func wait() async -> Result<ExecResult, Error> {
        for _ in 0 ..< 100 {
            let result = currentResult()
            if let result {
                return result
            }
            try? await Task.sleep(for: .milliseconds(20))
        }
        return .failure(ExecWaitError.timedOut)
    }

    private func currentResult() -> Result<ExecResult, Error>? {
        lock.lock()
        defer { lock.unlock() }
        return stored
    }
}

private enum ExecWaitError: Error {
    case timedOut
}
