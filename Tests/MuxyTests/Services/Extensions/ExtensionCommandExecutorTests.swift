import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionCommandExecutor", .serialized)
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

    @Test("cancellable exec matches synchronous result shape")
    func cancellableExecMatchesSynchronousResult() async throws {
        let request = ExecRequest(
            argv: nil,
            shell: "read value; printf 'out:%s' \"$value\"; printf 'err:%s' \"$value\" >&2; exit 7",
            cwd: nil,
            env: nil,
            stdin: "same",
            timeoutMs: 10000
        )
        let synchronous = try await ExtensionCommandExecutor.runUnchecked(
            request: request,
            extensionID: "test",
            defaultCwd: nil
        )
        let box = ExecCompletionBox()
        let jobID = ExtensionCommandExecutor.startCancelableUnchecked(
            request: request,
            extensionID: "test",
            defaultCwd: nil
        ) { result in
            box.complete(result)
        }
        defer { _ = ExtensionCommandExecutor.cancelExec(jobID: jobID, extensionID: "test") }
        let cancellable = try await box.wait().get()

        #expect(cancellable.stdout == synchronous.stdout)
        #expect(cancellable.stderr == synchronous.stderr)
        #expect(cancellable.exitCode == synchronous.exitCode)
        #expect(cancellable.timedOut == synchronous.timedOut)
        #expect(cancellable.truncated == synchronous.truncated)
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
            timeoutMs: 10000
        )
        let jobID = ExtensionCommandExecutor.startCancelableUnchecked(
            request: request,
            extensionID: "test",
            defaultCwd: nil
        ) { result in
            box.complete(result)
        }
        defer { _ = ExtensionCommandExecutor.cancelExec(jobID: jobID, extensionID: "test") }

        try await waitForFile(at: marker)
        #expect(ExtensionCommandExecutor.cancelExec(jobID: jobID, extensionID: "test"))

        do {
            _ = try await box.wait().get()
            Issue.record("expected cancellation")
        } catch ExecError.cancelled {
        } catch {
            Issue.record("expected ExecError.cancelled, got \(error)")
        }
    }

    @Test("cancelExec terminates descendants that inherit output pipes")
    func cancelExecTerminatesDescendants() async throws {
        let box = ExecCompletionBox()
        let childPIDFile = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-exec-child-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: childPIDFile) }
        let jobID = ExtensionCommandExecutor.startCancelableUnchecked(
            request: ExecRequest(
                argv: nil,
                shell: "/usr/bin/perl -e '$p=fork; if ($p == 0) { $SIG{TERM}=\"IGNORE\"; open O, \">\", $ARGV[0]; print O $$; close O; sleep 30; exit; } while (!-e $ARGV[0]) { select undef, undef, undef, 0.01; } sleep 30' \(childPIDFile.path)",
                cwd: nil,
                env: nil,
                stdin: nil,
                timeoutMs: 10000
            ),
            extensionID: "test",
            defaultCwd: nil
        ) { result in
            box.complete(result)
        }
        defer { _ = ExtensionCommandExecutor.cancelExec(jobID: jobID, extensionID: "test") }

        try await waitForFile(at: childPIDFile)
        let childPID = try #require(Int32(String(contentsOf: childPIDFile).trimmingCharacters(in: .whitespacesAndNewlines)))
        #expect(ExtensionCommandExecutor.cancelExec(jobID: jobID, extensionID: "test"))

        let result = await box.wait(milliseconds: 500)
        if case .failure(ExecError.cancelled) = result {
        } else {
            Issue.record("expected prompt cancellation, got \(result)")
        }
        #expect(await waitForProcessExit(childPID, milliseconds: 3000))
    }

    @Test("cancelExec by extension terminates its running jobs and leaves others running")
    func cancelExecByExtensionTerminatesMatchingJobs() async throws {
        let evictedBox = ExecCompletionBox()
        let survivorBox = ExecCompletionBox()
        let evictedMarker = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-exec-evicted-\(UUID().uuidString)")
        let survivorMarker = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-exec-survivor-\(UUID().uuidString)")
        let survivorRelease = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-exec-survivor-release-\(UUID().uuidString)")
        defer {
            try? FileManager.default.removeItem(at: evictedMarker)
            try? FileManager.default.removeItem(at: survivorMarker)
            try? FileManager.default.removeItem(at: survivorRelease)
        }

        let evictedJobID = ExtensionCommandExecutor.startCancelableUnchecked(
            request: ExecRequest(
                argv: nil,
                shell: "printf started > \(evictedMarker.path); while true; do sleep 1; done",
                cwd: nil,
                env: nil,
                stdin: nil,
                timeoutMs: 10000
            ),
            extensionID: "evicted-ext",
            defaultCwd: nil
        ) { result in
            evictedBox.complete(result)
        }
        defer { _ = ExtensionCommandExecutor.cancelExec(jobID: evictedJobID, extensionID: "evicted-ext") }
        let survivorJobID = ExtensionCommandExecutor.startCancelableUnchecked(
            request: ExecRequest(
                argv: nil,
                shell: "printf started > \(survivorMarker.path); while [ ! -f \(survivorRelease.path) ]; do sleep 0.02; done; printf survived",
                cwd: nil,
                env: nil,
                stdin: nil,
                timeoutMs: 10000
            ),
            extensionID: "survivor-ext",
            defaultCwd: nil
        ) { result in
            survivorBox.complete(result)
        }
        defer { _ = ExtensionCommandExecutor.cancelExec(jobID: survivorJobID, extensionID: "survivor-ext") }

        try await waitForFile(at: evictedMarker)
        try await waitForFile(at: survivorMarker)

        ExtensionCommandExecutor.cancelExec(extensionID: "evicted-ext")

        do {
            _ = try await evictedBox.wait().get()
            Issue.record("expected cancellation")
        } catch ExecError.cancelled {
        } catch {
            Issue.record("expected ExecError.cancelled, got \(error)")
        }
        #expect(!evictedJobID.isEmpty)
        try Data().write(to: survivorRelease)
        let survivor = try await survivorBox.wait().get()
        #expect(survivor.stdout == "survived")
        #expect(survivor.exitCode == 0)
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
        #expect(!ExtensionCommandExecutor.cancelExec(jobID: jobID, extensionID: "test"))
    }

    @Test("normal process exit wins before its callback is delivered")
    func normalExitWinsBeforeCallbackDelivery() async throws {
        let monitoringQueue = DispatchQueue(label: "muxy.exec.test.suspended-monitor")
        monitoringQueue.suspend()
        var monitoringQueueIsSuspended = true
        defer {
            if monitoringQueueIsSuspended {
                monitoringQueue.resume()
            }
        }

        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/true")
        let stdinPipe = Pipe()
        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()
        let completion = ProcessCompletionBox()
        let runningProcess = try CancellableProcess.launch(
            configuredProcess: process,
            stdinPipe: stdinPipe,
            stdoutPipe: stdoutPipe,
            stderrPipe: stderrPipe,
            monitoringQueue: monitoringQueue
        ) {
            completion.complete()
        }
        try? stdinPipe.fileHandleForWriting.close()

        try await waitForUnreapedProcessExit(runningProcess.processIdentifier)
        #expect(!runningProcess.terminate())
        #expect(await completion.wait())
        #expect(runningProcess.terminationStatus == 0)

        monitoringQueue.resume()
        monitoringQueueIsSuspended = false
    }

    @Test("cancelled owner state prevents authorization and launch")
    func cancelledOwnerStatePreventsStart() async {
        let box = ExecCompletionBox()
        let jobID = ExtensionCommandExecutor.startCancelableExec(
            request: ExecRequest(
                argv: ["/bin/sleep", "30"],
                shell: nil,
                cwd: nil,
                env: nil,
                stdin: nil,
                timeoutMs: 10000
            ),
            extensionID: "cancelled-owner",
            defaultCwd: nil,
            isCancelled: { true }
        ) { result in
            box.complete(result)
        }
        defer { _ = ExtensionCommandExecutor.cancelExec(jobID: jobID, extensionID: "cancelled-owner") }

        if case .failure(ExecError.cancelled) = await box.wait() {
        } else {
            Issue.record("expected cancellation before authorization")
        }
        #expect(!ExtensionCommandExecutor.cancelExec(jobID: jobID, extensionID: "cancelled-owner"))
    }

    @Test("accepted cancellation wins an authorization failure race")
    func acceptedCancellationWinsAuthorizationFailureRace() async {
        let box = ExecCompletionBox()
        let gate = AuthorizationFailureGate()
        let cancellationGate = CancellationClaimGate()
        let jobID = ExtensionCommandExecutor.startCancelableExec(
            request: ExecRequest(
                argv: ["/bin/echo", "never-runs"],
                shell: nil,
                cwd: nil,
                env: nil,
                stdin: nil,
                timeoutMs: nil
            ),
            extensionID: "test-extension",
            defaultCwd: nil,
            onCancellationClaimed: {
                cancellationGate.claimAndWait()
            },
            authorize: { _, _ in
                try await gate.authorize()
            }
        ) { result in
            box.complete(result)
        }

        await gate.waitUntilStarted()
        let cancellation = Task.detached {
            ExtensionCommandExecutor.cancelExec(jobID: jobID, extensionID: "test-extension")
        }
        await Task.detached {
            cancellationGate.waitUntilClaimed()
        }.value
        await gate.release()
        await gate.waitUntilFinished()

        let result = await box.wait()
        cancellationGate.release()
        #expect(await cancellation.value)

        if case .failure(ExecError.cancelled) = result {
        } else {
            Issue.record("accepted cancellation must produce ExecError.cancelled")
        }
        #expect(box.count == 1)
    }

    @Test("cancellable exec rejects a cwd containing null bytes")
    func cancellableExecRejectsNullCwd() async {
        let request = ExecRequest(
            argv: ["/usr/bin/true"],
            shell: nil,
            cwd: "/tmp\0/ignored",
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
            Issue.record("expected synchronous invalid cwd rejection")
        } catch let ExecError.invalidArguments(message) {
            #expect(message.contains("cwd"))
        } catch {
            Issue.record("expected synchronous ExecError.invalidArguments, got \(error)")
        }

        let box = ExecCompletionBox()
        _ = ExtensionCommandExecutor.startCancelableUnchecked(
            request: request,
            extensionID: "test",
            defaultCwd: nil
        ) { result in
            box.complete(result)
        }

        do {
            _ = try await box.wait().get()
            Issue.record("expected invalid cwd rejection")
        } catch let ExecError.invalidArguments(message) {
            #expect(message.contains("cwd"))
        } catch {
            Issue.record("expected ExecError.invalidArguments, got \(error)")
        }
    }

    @Test("cancelExec rejects a job owned by another extension")
    func cancelExecChecksOwnership() async throws {
        let box = ExecCompletionBox()
        let marker = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-exec-owned-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: marker) }
        let jobID = ExtensionCommandExecutor.startCancelableUnchecked(
            request: ExecRequest(
                argv: nil,
                shell: "printf started > \(marker.path); while true; do sleep 1; done",
                cwd: nil,
                env: nil,
                stdin: nil,
                timeoutMs: 10000
            ),
            extensionID: "owner",
            defaultCwd: nil
        ) { result in
            box.complete(result)
        }
        defer { _ = ExtensionCommandExecutor.cancelExec(jobID: jobID, extensionID: "owner") }

        try await waitForFile(at: marker)
        #expect(!ExtensionCommandExecutor.cancelExec(jobID: jobID, extensionID: "other"))
        #expect(box.count == 0)
        #expect(ExtensionCommandExecutor.cancelExec(jobID: jobID, extensionID: "owner"))
        if case .failure(ExecError.cancelled) = await box.wait() {
        } else {
            Issue.record("expected owner cancellation")
        }
    }

    @Test("cancel after timeout does not replace the timeout result")
    func cancelAfterTimeoutDoesNotReplaceResult() async throws {
        let box = ExecCompletionBox()
        let marker = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-exec-timeout-started-\(UUID().uuidString)")
        let timeoutMarker = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-exec-timeout-fired-\(UUID().uuidString)")
        defer {
            try? FileManager.default.removeItem(at: marker)
            try? FileManager.default.removeItem(at: timeoutMarker)
        }
        let jobID = ExtensionCommandExecutor.startCancelableUnchecked(
            request: ExecRequest(
                argv: nil,
                shell: "trap 'printf timeout > \(timeoutMarker.path)' TERM; printf started > \(marker.path); while true; do sleep 1; done",
                cwd: nil,
                env: nil,
                stdin: nil,
                timeoutMs: 200
            ),
            extensionID: "test",
            defaultCwd: nil
        ) { result in
            box.complete(result)
        }

        try await waitForFile(at: marker)
        try await waitForFile(at: timeoutMarker)
        #expect(!ExtensionCommandExecutor.cancelExec(jobID: jobID, extensionID: "test"))
        let result = try await box.wait().get()
        #expect(result.timedOut)
        #expect(result.exitCode != 0)
        #expect(box.count == 1)
    }

    @Test("timeout remains active while writing stdin")
    func timeoutWhileWritingStdin() async throws {
        let box = ExecCompletionBox()
        let clock = ContinuousClock()
        let started = clock.now
        _ = ExtensionCommandExecutor.startCancelableUnchecked(
            request: ExecRequest(
                argv: ["/bin/sleep", "15"],
                shell: nil,
                cwd: nil,
                env: nil,
                stdin: String(repeating: "x", count: 10 * 1024 * 1024),
                timeoutMs: 100
            ),
            extensionID: "test",
            defaultCwd: nil
        ) { result in
            box.complete(result)
        }

        let result = try await box.wait(milliseconds: 10000).get()
        #expect(result.timedOut)
        #expect(result.exitCode != 0)
        #expect(clock.now - started < .seconds(10))
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

    @Test("concurrent jobs beyond the per-extension limit are rejected")
    func concurrentJobLimitIsEnforced() async throws {
        let limit = ExtensionCommandExecutor.maxConcurrentJobsPerExtension
        let request = ExecRequest(
            argv: ["/bin/sleep", "30"],
            shell: nil,
            cwd: nil,
            env: nil,
            stdin: nil,
            timeoutMs: 10000
        )
        var jobIDs: [String] = []
        defer {
            for jobID in jobIDs {
                _ = ExtensionCommandExecutor.cancelExec(jobID: jobID, extensionID: "capped")
            }
        }
        for _ in 0 ..< limit {
            jobIDs.append(ExtensionCommandExecutor.startCancelableUnchecked(
                request: request,
                extensionID: "capped",
                defaultCwd: nil
            ) { _ in })
        }

        let rejected = ExecCompletionBox()
        _ = ExtensionCommandExecutor.startCancelableUnchecked(
            request: request,
            extensionID: "capped",
            defaultCwd: nil
        ) { result in
            rejected.complete(result)
        }

        if case let .failure(ExecError.tooManyConcurrentCommands(reported)) = await rejected.wait() {
            #expect(reported == limit)
        } else {
            Issue.record("expected the concurrency limit to reject the extra job")
        }

        #expect(ExtensionCommandExecutor.cancelExec(jobID: jobIDs.removeLast(), extensionID: "capped"))
        #expect(try await slotIsReleased(extensionID: "capped"))
    }

    @Test("a different extension is unaffected by another extension's limit")
    func concurrentJobLimitIsPerExtension() async throws {
        let limit = ExtensionCommandExecutor.maxConcurrentJobsPerExtension
        let blocking = ExecRequest(
            argv: ["/bin/sleep", "30"],
            shell: nil,
            cwd: nil,
            env: nil,
            stdin: nil,
            timeoutMs: 10000
        )
        var jobIDs: [String] = []
        defer {
            for jobID in jobIDs {
                _ = ExtensionCommandExecutor.cancelExec(jobID: jobID, extensionID: "noisy")
            }
        }
        for _ in 0 ..< limit {
            jobIDs.append(ExtensionCommandExecutor.startCancelableUnchecked(
                request: blocking,
                extensionID: "noisy",
                defaultCwd: nil
            ) { _ in })
        }

        let quiet = ExecCompletionBox()
        _ = ExtensionCommandExecutor.startCancelableUnchecked(
            request: ExecRequest(
                argv: ["/bin/echo", "quiet"],
                shell: nil,
                cwd: nil,
                env: nil,
                stdin: nil,
                timeoutMs: nil
            ),
            extensionID: "quiet",
            defaultCwd: nil
        ) { result in
            quiet.complete(result)
        }

        let result = try await quiet.wait().get()
        #expect(result.exitCode == 0)
        #expect(result.stdout.contains("quiet"))
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

private func slotIsReleased(extensionID: String) async throws -> Bool {
    for _ in 0 ..< 100 {
        let box = ExecCompletionBox()
        _ = ExtensionCommandExecutor.startCancelableUnchecked(
            request: ExecRequest(
                argv: ["/usr/bin/true"],
                shell: nil,
                cwd: nil,
                env: nil,
                stdin: nil,
                timeoutMs: nil
            ),
            extensionID: extensionID,
            defaultCwd: nil
        ) { result in
            box.complete(result)
        }
        guard case .failure(ExecError.tooManyConcurrentCommands) = await box.wait() else { return true }
        try await Task.sleep(for: .milliseconds(20))
    }
    return false
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

private func waitForProcessExit(_ pid: pid_t, milliseconds: Int = 1000) async -> Bool {
    for _ in 0 ..< max(1, milliseconds / 20) {
        if kill(pid, 0) == -1, errno == ESRCH {
            return true
        }
        var info = proc_bsdinfo()
        let infoSize = Int32(MemoryLayout.size(ofValue: info))
        if proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, &info, infoSize) == infoSize,
           info.pbi_status == SZOMB
        {
            return true
        }
        try? await Task.sleep(for: .milliseconds(20))
    }
    return false
}

private func waitForUnreapedProcessExit(_ pid: pid_t) async throws {
    for _ in 0 ..< 100 {
        var info = siginfo_t()
        if waitid(P_PID, id_t(pid), &info, WEXITED | WNOHANG | WNOWAIT) == 0,
           info.si_pid == pid
        {
            return
        }
        try await Task.sleep(for: .milliseconds(20))
    }
    throw ExecWaitError.timedOut
}

private final class ProcessCompletionBox: @unchecked Sendable {
    private let lock = NSLock()
    private var completed = false

    func complete() {
        lock.lock()
        completed = true
        lock.unlock()
    }

    func wait() async -> Bool {
        for _ in 0 ..< 100 {
            if isCompleted() { return true }
            try? await Task.sleep(for: .milliseconds(20))
        }
        return false
    }

    private func isCompleted() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        return completed
    }
}

private actor AuthorizationFailureGate {
    private var started = false
    private var released = false
    private var finished = false
    private var startedWaiters: [CheckedContinuation<Void, Never>] = []
    private var releaseWaiters: [CheckedContinuation<Void, Never>] = []
    private var finishedWaiters: [CheckedContinuation<Void, Never>] = []

    func authorize() async throws -> WorkspaceContext {
        started = true
        startedWaiters.forEach { $0.resume() }
        startedWaiters.removeAll()
        if !released {
            await withCheckedContinuation { releaseWaiters.append($0) }
        }
        finished = true
        finishedWaiters.forEach { $0.resume() }
        finishedWaiters.removeAll()
        throw ExecError.invalidArguments("forced authorization failure")
    }

    func waitUntilStarted() async {
        guard !started else { return }
        await withCheckedContinuation { startedWaiters.append($0) }
    }

    func release() {
        released = true
        releaseWaiters.forEach { $0.resume() }
        releaseWaiters.removeAll()
    }

    func waitUntilFinished() async {
        guard !finished else { return }
        await withCheckedContinuation { finishedWaiters.append($0) }
    }
}

private final class CancellationClaimGate: @unchecked Sendable {
    private let claimed = DispatchSemaphore(value: 0)
    private let released = DispatchSemaphore(value: 0)

    func claimAndWait() {
        claimed.signal()
        released.wait()
    }

    func waitUntilClaimed() {
        claimed.wait()
    }

    func release() {
        released.signal()
    }
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

    func wait(milliseconds: Int = 2000) async -> Result<ExecResult, Error> {
        for _ in 0 ..< max(1, milliseconds / 20) {
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
