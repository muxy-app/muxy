import Darwin
import Foundation
import Testing

@testable import Muxy

@Suite("WorktreeProcessQuiescer")
struct WorktreeProcessQuiescerTests {
    @Test("matches only processes working inside the worktree")
    func matchesProcessesInsideWorktree() {
        let directories: [pid_t: String] = [
            10: "/tmp/repo-feature",
            11: "/tmp/repo-feature/generated",
            12: "/tmp/repo-feature-other",
            13: "/tmp/repo",
        ]

        let processIDs = WorktreeProcessQuiescer.matchingProcessIDs(
            using: "/tmp/repo-feature",
            candidates: [10, 11, 12, 13],
            currentProcessID: 99,
            workingDirectory: { directories[$0] }
        )

        #expect(processIDs == [10, 11])
    }

    @Test("excludes the current process")
    func excludesCurrentProcess() {
        let processIDs = WorktreeProcessQuiescer.matchingProcessIDs(
            using: "/tmp/repo-feature",
            candidates: [10],
            currentProcessID: 10,
            workingDirectory: { _ in "/tmp/repo-feature" }
        )

        #expect(processIDs.isEmpty)
    }

    @Test("rejects a symlink root before signaling")
    func rejectsSymlinkRootBeforeSignaling() async throws {
        let root = try makeDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let link = root.appendingPathComponent("link")
        try FileManager.default.createSymbolicLink(at: link, withDestinationURL: root)
        let recorder = SignalRecorder()
        let identity = try #require(WorktreeProcessQuiescer.directoryIdentity(at: root.path))

        await #expect(throws: WorktreeProcessQuiescerError.self) {
            try await WorktreeProcessQuiescer.quiesce(
                path: link.path,
                matching: identity,
                timeout: 1,
                processIDs: { [42] },
                workingDirectory: { _ in root.path },
                sendSignal: { processID, signal in
                    recorder.record(processID: processID, signal: signal)
                    return 0
                }
            )
        }

        #expect(recorder.signals.isEmpty)
    }

    @Test("rejects a replaced root immediately before signaling")
    func rejectsReplacementBeforeSignaling() async throws {
        let root = try makeDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let target = root.appendingPathComponent("worktree")
        try FileManager.default.createDirectory(at: target, withIntermediateDirectories: true)
        let recorder = SignalRecorder()
        let identity = try #require(WorktreeProcessQuiescer.directoryIdentity(at: target.path))

        await #expect(throws: WorktreeProcessQuiescerError.self) {
            try await WorktreeProcessQuiescer.quiesce(
                path: target.path,
                matching: identity,
                timeout: 1,
                processIDs: { [42] },
                workingDirectory: { _ in
                    try? FileManager.default.removeItem(at: target)
                    try? FileManager.default.createDirectory(at: target, withIntermediateDirectories: true)
                    return target.path
                },
                sendSignal: { processID, signal in
                    recorder.record(processID: processID, signal: signal)
                    return 0
                }
            )
        }

        #expect(recorder.signals.isEmpty)
    }

    @Test("restores a quarantined directory when bounded deletion fails")
    func restoresDirectoryWhenDeletionFails() async throws {
        let root = try makeDirectory()
        defer { try? FileManager.default.removeItem(at: root) }
        let target = root.appendingPathComponent("worktree")
        try FileManager.default.createDirectory(at: target, withIntermediateDirectories: true)
        let identity = try #require(WorktreeProcessQuiescer.directoryIdentity(at: target.path))

        let recorder = RemovalRecorder()

        await #expect(throws: AsyncTimeoutError.self) {
            try await WorktreeProcessQuiescer.removeDirectory(
                at: target.path,
                matching: identity,
                timeout: 7,
                boundedRemover: { path, timeout in
                    recorder.record(path: path, timeout: timeout)
                    throw AsyncTimeoutError.timedOut(timeout)
                }
            )
        }

        #expect(WorktreeProcessQuiescer.directoryIdentity(at: target.path) == identity)
        #expect(recorder.timeout == 7)
    }

    private func makeDirectory() throws -> URL {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-quiescer-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        return root
    }
}

private final class SignalRecorder: @unchecked Sendable {
    private(set) var signals: [(pid_t, Int32)] = []

    func record(processID: pid_t, signal: Int32) {
        signals.append((processID, signal))
    }
}

private final class RemovalRecorder: @unchecked Sendable {
    private(set) var path: String?
    private(set) var timeout: TimeInterval?

    func record(path: String, timeout: TimeInterval) {
        self.path = path
        self.timeout = timeout
    }
}
