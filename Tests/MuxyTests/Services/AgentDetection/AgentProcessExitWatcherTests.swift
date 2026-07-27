import Foundation
import Testing

@testable import Muxy

@Suite("AgentProcessExitWatcher", .timeLimit(.minutes(1)))
@MainActor
struct AgentProcessExitWatcherTests {
    @Test("reports the exit of a watched process")
    func reportsExit() async throws {
        let process = try startSleepProcess()
        let watcher = AgentProcessExitWatcher()

        await withCheckedContinuation { continuation in
            watcher.onExit = { continuation.resume() }
            watcher.watch(processID: process.processIdentifier)
            process.terminate()
        }

        #expect(watcher.processID == nil)
    }

    @Test("cancelling stops reporting the exit")
    func cancelStopsReporting() async throws {
        let process = try startSleepProcess()
        let watcher = AgentProcessExitWatcher()
        var reported = false
        watcher.onExit = { reported = true }
        watcher.watch(processID: process.processIdentifier)

        watcher.cancel()
        process.terminate()
        process.waitUntilExit()
        try await Task.sleep(for: .milliseconds(200))

        #expect(!reported)
        #expect(watcher.processID == nil)
    }

    @Test("watching the same process twice keeps a single watch")
    func repeatedWatchKeepsProcess() throws {
        let process = try startSleepProcess()
        defer { process.terminate() }
        let watcher = AgentProcessExitWatcher()

        watcher.watch(processID: process.processIdentifier)
        watcher.watch(processID: process.processIdentifier)

        #expect(watcher.processID == process.processIdentifier)
    }

    private func startSleepProcess() throws -> Process {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/sleep")
        process.arguments = ["30"]
        try process.run()
        return process
    }
}
