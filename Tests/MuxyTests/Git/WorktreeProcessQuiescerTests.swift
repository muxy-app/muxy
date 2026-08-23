import Darwin
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
}
