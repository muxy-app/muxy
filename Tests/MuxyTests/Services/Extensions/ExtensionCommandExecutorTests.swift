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
